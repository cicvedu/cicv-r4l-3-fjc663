// SPDX-License-Identifier: GPL-2.0

//! Rust for linux e1000 driver demo

#![allow(unused)]

// 导入核心库中的迭代器模块和原子指针模块
use core::iter::Iterator;
use core::sync::atomic::AtomicPtr;

// 导入内核模块及其相关依赖
use kernel::pci::Resource;
use kernel::prelude::*;
use kernel::sync::Arc;
use kernel::{pci, device, driver, bindings, net, dma, c_str};
use kernel::device::RawDevice;
use kernel::sync::SpinLock;

// 导入自定义模块
mod consts;
mod hw_defs;
mod ring_buf;
mod e1000_ops;

// 从 hw_defs 模块导入 TxDescEntry 和 RxDescEntry
use hw_defs::{TxDescEntry, RxDescEntry};
// 从 ring_buf 模块导入 RxRingBuf 和 TxRingBuf
use ring_buf::{RxRingBuf, TxRingBuf};

// 从 e1000_ops 模块导入 E1000Ops
use e1000_ops::E1000Ops;

// 从 consts 模块导入常量
use consts::*;

// 定义内核模块信息
module! {
    type: E1000KernelMod,
    name: "r4l_e1000_demo",
    author: "Myrfy001",
    description: "Rust for linux e1000 driver demo",
    license: "GPL",
}

/// 该驱动程序的私有数据结构
struct NetDevicePrvData {
    dev: Arc<device::Device>,  // 设备的引用计数指针
    napi: Arc<net::Napi>,  // NAPI 结构的引用计数指针
    e1000_hw_ops: Arc<E1000Ops>,  // e1000 硬件操作的引用计数指针
    tx_ring: SpinLock<Option<TxRingBuf>>,  // 发送环形缓冲区的自旋锁
    rx_ring: SpinLock<Option<RxRingBuf>>,  // 接收环形缓冲区的自旋锁
    irq: u32,  // 中断请求编号
    _irq_handler: AtomicPtr<kernel::irq::Registration<E1000InterruptHandler>>,  // 中断处理程序的原子指针
    pci_dev: Arc<*mut bindings::pci_dev>, // pci_dev指针
}

// 声明 NetDevicePrvData 结构体可以安全地在多线程中传递和共享
unsafe impl Send for NetDevicePrvData {}
unsafe impl Sync for NetDevicePrvData {}

/// 表示网络设备的结构体
struct NetDevice {}

impl NetDevice {

    /// 分配发送描述符资源。但不需要分配缓冲区内存，因为网络栈会传递一个 SkBuff。
    fn e1000_setup_all_tx_resources(data: &NetDevicePrvData) -> Result<TxRingBuf> {

        // 为发送描述符分配 DMA 内存空间
        // dma::Allocation 是一个泛型结构体，这里指定了 TxDescEntry 类型
        // TX_RING_SIZE 是发送环形缓冲区的大小，bindings::GFP_KERNEL 表示分配内存的标志
        let dma_desc = dma::Allocation::<hw_defs::TxDescEntry>::try_new(&*data.dev, TX_RING_SIZE, bindings::GFP_KERNEL)?;

        // 安全：从原始指针创建可变切片，大小为 TX_RING_SIZE
        // 所有切片成员的字段将在下面初始化，因此这是安全的
        let tx_ring = unsafe { core::slice::from_raw_parts_mut(dma_desc.cpu_addr, TX_RING_SIZE) };

        // 初始化发送描述符环形缓冲区中的每个描述符
        tx_ring.iter_mut().enumerate().for_each(|(idx, desc)| {
            desc.buf_addr = 0;     // 缓冲区地址，初始为0
            desc.cmd = 0;          // 命令字段，初始为0
            desc.length = 0;       // 数据长度，初始为0
            desc.cso = 0;          // 校验和偏移，初始为0
            desc.css = 0;          // 校验和起始，初始为0
            desc.special = 0;      // 特殊字段，初始为0
            desc.sta = E1000_TXD_STAT_DD as u8;  // 标记所有描述符为已完成状态，使得第一个数据包可以传输
        });

        // 创建并返回一个新的 TxRingBuf 实例
        Ok(TxRingBuf::new(dma_desc, TX_RING_SIZE))
    }

    /// 分配接收描述符和相应的内存空间。使用 `alloc_skb_ip_align` 分配缓冲区，然后将其映射到 DMA 地址。
    fn e1000_setup_all_rx_resources(dev: &net::Device, data: &NetDevicePrvData) -> Result<RxRingBuf> {

        // 为接收描述符分配 DMA 内存空间
        // dma::Allocation 是一个泛型结构体，这里指定了 RxDescEntry 类型
        // RX_RING_SIZE 是接收环形缓冲区的大小，bindings::GFP_KERNEL 表示分配内存的标志
        let dma_desc = dma::Allocation::<hw_defs::RxDescEntry>::try_new(&*data.dev, RX_RING_SIZE, bindings::GFP_KERNEL)?;

        // 安全：从原始指针创建可变切片，大小为 RX_RING_SIZE
        // 所有切片成员的字段将在下面初始化，因此这是安全的
        let rx_ring_desc = unsafe { core::slice::from_raw_parts_mut(dma_desc.cpu_addr, RX_RING_SIZE) };

        // 为接收缓冲区分配 DMA 内存空间
        // dma::Allocation 是一个泛型结构体，这里指定了 u8 类型
        // RX_RING_SIZE * RXTX_SINGLE_RING_BLOCK_SIZE 表示分配的总大小
        let dma_buf = dma::Allocation::<u8>::try_new(&*data.dev, RX_RING_SIZE * RXTX_SINGLE_RING_BLOCK_SIZE, bindings::GFP_KERNEL)?;

        // 创建一个新的 RxRingBuf 实例
        let mut rx_ring = RxRingBuf::new(dma_desc, RX_RING_SIZE);

        // 初始化接收描述符环形缓冲区中的每个描述符
        rx_ring_desc.iter_mut().enumerate().for_each(|(idx, desc)| {
            // 分配一个新的 SkBuff，大小为 RXTX_SINGLE_RING_BLOCK_SIZE
            let skb = dev.alloc_skb_ip_align(RXTX_SINGLE_RING_BLOCK_SIZE as u32).unwrap();
            // 将 SkBuff 映射到 DMA 地址
            let dma_map = dma::MapSingle::try_new(&*data.dev, skb.head_data().as_ptr() as *mut u8, RXTX_SINGLE_RING_BLOCK_SIZE, bindings::dma_data_direction_DMA_FROM_DEVICE).unwrap();

            // 初始化描述符字段
            desc.buf_addr = dma_map.dma_handle as u64;  // 设置缓冲区地址为 DMA 映射的地址
            desc.length = 0;       // 数据长度，初始为0
            desc.special = 0;      // 特殊字段，初始为0
            desc.checksum = 0;     // 校验和，初始为0
            desc.status = 0;       // 状态，初始为0
            desc.errors = 0;       // 错误，初始为0

            // 将 DMA 映射和 SkBuff 存储在接收环形缓冲区中
            rx_ring.buf.borrow_mut()[idx] = Some((dma_map, skb));
        });

        // 返回初始化好的接收环形缓冲区
        Ok(rx_ring)
    }

    // 对应于 C 版本的 e1000_clean_tx_irq()，用于回收发送队列中的描述符
    fn e1000_recycle_tx_queue(dev: &net::Device, data: &NetDevicePrvData) {
        // 读取发送队列尾部指针
        let tdt = data.e1000_hw_ops.e1000_read_tx_queue_tail();
        // 读取发送队列头部指针
        let tdh = data.e1000_hw_ops.e1000_read_tx_queue_head();

        // 获取发送环形缓冲区的锁并禁用中断
        let mut tx_ring = data.tx_ring.lock_irqdisable();
        // 确保发送环形缓冲区存在
        let mut tx_ring = tx_ring.as_mut().unwrap();

        // 获取发送描述符的切片
        let descs = tx_ring.desc.as_desc_slice();

        // 获取下一个要清理的描述符索引
        let mut idx = tx_ring.next_to_clean;
        // 循环遍历发送描述符，回收已完成的描述符
        while descs[idx].sta & E1000_TXD_STAT_DD as u8 != 0 && idx != tdh as usize {
            // 取出并丢弃 DMA 映射和 SkBuff
            let (dm, skb) = tx_ring.buf.borrow_mut()[idx].take().unwrap();
            // 更新已完成队列的统计信息
            dev.completed_queue(1, skb.len());
            // 消耗 napi
            skb.napi_consume(64);
            drop(dm);  // 释放 DMA 映射
            drop(skb);  // 释放 SkBuff

            // 更新索引
            idx = (idx + 1) % TX_RING_SIZE;
        }

        // 更新环形缓冲区的下一个清理索引
        tx_ring.next_to_clean = idx;
    }
}

#[vtable]
impl net::DeviceOperations for NetDevice {

    type Data = Box<NetDevicePrvData>;

    /// 当你在 shell 中输入 ip link set eth0 up 时，这个方法会被调用。
    fn open(dev: &net::Device, data: &NetDevicePrvData) -> Result {
        pr_info!("Rust for linux e1000 driver demo (net device open)\n");

        // 关闭网络接口的 carrier
        dev.netif_carrier_off();

        // 初始化用于传输（TX）和接收（RX）的 DMA 内存
        let tx_ringbuf = Self::e1000_setup_all_tx_resources(data)?;
        let rx_ringbuf = Self::e1000_setup_all_rx_resources(dev, data)?;

        // TODO: e1000_power_up_phy() 方法尚未实现。此方法用于在 PHY 可能处于关闭状态时进行电源恢复，
        // 但在这个最小可行产品（MVP）驱动程序中不支持该功能。

        // 修改 e1000 硬件寄存器，向网卡提供 RX/TX 队列信息
        data.e1000_hw_ops.e1000_configure(&rx_ringbuf, &tx_ringbuf)?;

        // 将接收（RX）和传输（TX）队列的锁定状态存储到数据结构中
        *data.rx_ring.lock_irqdisable() = Some(rx_ringbuf);
        *data.tx_ring.lock_irqdisable() = Some(tx_ringbuf);

        // 创建 IRQ 处理程序的私有数据
        let irq_prv_data = Box::try_new(IrqPrivateData{
            e1000_hw_ops: Arc::clone(&data.e1000_hw_ops),
            napi: Arc::clone(&data.napi),
        })?;

        // 创建 IRQ 注册对象。注意 irq::Registration 包含一个实现了 Drop trait 的 irq::InternalRegistration，
        // 因此我们必须确保它不会被释放。
        // TODO: 目前存在内存泄漏问题。
        let req_reg = kernel::irq::Registration::<E1000InterruptHandler>::try_new(
            data.irq,
            irq_prv_data,
            kernel::irq::flags::SHARED,
            fmt!("{}", data.dev.name())
        )?;

        data._irq_handler.store(Box::into_raw(Box::try_new(req_reg)?), core::sync::atomic::Ordering::Relaxed);

        // 启用 NAPI（New API）以处理网络中断
        data.napi.enable();

        // 启动网络接口队列
        dev.netif_start_queue();

        // 启用网络接口的 carrier
        dev.netif_carrier_on();

        Ok(())
    }

    // 停止网络设备的操作
    fn stop(_dev: &net::Device, _data: &NetDevicePrvData) -> Result {
        pr_info!("Rust for linux e1000 driver demo (net device stop)\n");
        Ok(())
    }

    // 处理网络数据包的发送
    fn start_xmit(skb: &net::SkBuff, dev: &net::Device, data: &NetDevicePrvData) -> net::NetdevTx {

        // 如果数据包大小超过单个 RX/TX 环形缓冲区的大小，打印错误信息并返回忙碌状态
        if skb.head_data().len() > RXTX_SINGLE_RING_BLOCK_SIZE {
            pr_err!("xmit msg too long");
            return net::NetdevTx::Busy;
        }

        // 获取传输（TX）环形缓冲区
        let mut tx_ring = data.tx_ring.lock_irqdisable();
        // 读取 TX 队列的尾部和头部索引，以及 RX 队列的尾部和头部索引
        let mut tdt = data.e1000_hw_ops.e1000_read_tx_queue_tail();
        let tdh = data.e1000_hw_ops.e1000_read_tx_queue_head();
        let rdt = data.e1000_hw_ops.e1000_read_rx_queue_tail();
        let rdh = data.e1000_hw_ops.e1000_read_rx_queue_head();

        pr_info!("Rust for linux e1000 driver demo (net device start_xmit) tdt={}, tdh={}, rdt={}, rdh={}\n", tdt, tdh, rdt, rdh);

        // 在 PCI/PCI-X 硬件上，如果数据包大小小于 ETH_ZLEN，数据包在硬件填充过程中可能会被破坏。
        // 为了避免这个问题，手动填充所有小数据包。
        skb.put_padto(bindings::ETH_ZLEN);

        // 告诉内核我们已经将数据提交到硬件
        dev.sent_queue(skb.len());

        let mut tx_ring = tx_ring.as_mut().unwrap();
        // 获取 TX 描述符数组中的描述符
        let tx_descs: &mut [TxDescEntry] = tx_ring.desc.as_desc_slice();
        // 获取当前的 TX 描述符
        let tx_desc = &mut tx_descs[tdt as usize];
        // 检查 TX 描述符的状态位，如果描述符不可用，则打印错误信息并返回忙碌状态
        if tx_desc.sta & E1000_TXD_STAT_DD as u8 == 0 {
            pr_err!("xmit busy");
            return net::NetdevTx::Busy;
        }

        // 为 skb 分配 DMA 映射
        let ms: dma::MapSingle<u8> = if let Ok(ms) = dma::MapSingle::try_new(
            &*data.dev,
            skb.head_data().as_ptr() as *mut u8,
            skb.len() as usize,
            bindings::dma_data_direction_DMA_TO_DEVICE
        ) {
            ms
        } else {
            return net::NetdevTx::Busy;
        };

        // 更新 TX 描述符的缓冲区地址、长度和命令
        tx_desc.buf_addr = ms.dma_handle as u64;
        tx_desc.length = skb.len() as u16;
        tx_desc.cmd = ((E1000_TXD_CMD_RS | E1000_TXD_CMD_EOP) >> 24) as u8;
        tx_desc.sta = 0;
        // 将 DMA 映射和 skb 存储到 TX 环形缓冲区中
        tx_ring.buf.borrow_mut()[tdt as usize].replace((ms, skb.into()));

        // TODO: 在这里可能需要内存屏障。我们在 x86 上进行测试，因此可以忽略这一步。

        // 更新 TX 队列尾部索引
        tdt = (tdt + 1) % TX_RING_SIZE as u32;
        data.e1000_hw_ops.e1000_write_tx_queue_tail(tdt);

        net::NetdevTx::Ok
    }

    // 获取网络设备的统计信息
    fn get_stats64(_netdev: &net::Device, _data: &NetDevicePrvData, stats: &mut net::RtnlLinkStats64) {
        pr_info!("Rust for linux e1000 driver demo (net device get_stats64)\n");
        // TODO: 尚未实现统计信息的获取
        stats.set_rx_bytes(0);
        stats.set_rx_packets(0);
        stats.set_tx_bytes(0);
        stats.set_tx_packets(0);
    }
}


// 由于所有权限制，我们不能直接使用 C 代码中的 NetDevicePrvData 类型，因此需要在此定义一个新的类型。
struct IrqPrivateData {
    // E1000 硬件操作结构体的引用，使用 Arc 进行线程安全的共享
    e1000_hw_ops: Arc<E1000Ops>,
    // NAPI（网络设备轮询接口）的引用，使用 Arc 进行线程安全的共享
    napi: Arc<net::Napi>,
}

// 中断处理器结构体
struct E1000InterruptHandler {}

impl kernel::irq::Handler for E1000InterruptHandler {
    // 中断处理器的数据类型是 Box<IrqPrivateData>
    type Data = Box<IrqPrivateData>;

    // 处理中断的逻辑
    fn handle_irq(data: &IrqPrivateData) -> kernel::irq::Return {
        // 打印日志，表明中断处理程序被调用
        pr_info!("Rust for linux e1000 driver demo (handle_irq)\n");

        // 读取当前中断状态
        let pending_irqs = data.e1000_hw_ops.e1000_read_interrupt_state();

        // 打印待处理的中断标志
        pr_info!("pending_irqs: {}\n", pending_irqs);

        // 如果没有待处理的中断，则返回 None
        if pending_irqs == 0 {
            return kernel::irq::Return::None;
        }

        // 如果有待处理的中断，则调度 NAPI 进行处理
        data.napi.schedule();

        // 返回中断处理完成的标志
        kernel::irq::Return::Handled
    }
}



// 定义用于管理网络设备注册信息的结构体
struct E1000DrvPrvData {
    // 网络设备的注册信息
    _netdev_reg: net::Registration<NetDevice>,
}

// 实现 `driver::DeviceRemoval` 特征，用于处理设备移除事件
impl driver::DeviceRemoval for E1000DrvPrvData {
    fn device_remove(&self) {
        // 打印日志，表明设备正在被移除
        pr_info!("Rust for linux e1000 driver demo (device_remove)\n");
    }
}

// 定义 NAPI 轮询处理程序的结构体
struct NapiHandler {}

// 实现 `net::NapiPoller` 特征，用于处理 NAPI 的轮询事件
impl net::NapiPoller for NapiHandler {
    // 定义与 `NetDevicePrvData` 类型相关的数据
    type Data = Box<NetDevicePrvData>;

    // 实现轮询逻辑
    fn poll(
        _napi: &net::Napi,
        _budget: i32,
        dev: &net::Device,
        data: &NetDevicePrvData,
    ) -> i32 {
        // 打印日志，表明 NAPI 正在进行轮询
        pr_info!("Rust for linux e1000 driver demo (napi poll)\n");

        // 读取接收队列的尾部索引，并更新为下一个索引
        let mut rdt = data.e1000_hw_ops.e1000_read_rx_queue_tail() as usize;
        rdt = (rdt + 1) % RX_RING_SIZE;

        // 锁定接收环形缓冲区
        let mut rx_ring_guard = data.rx_ring.lock();
        let rx_ring = rx_ring_guard.as_mut().unwrap();

        // 获取接收描述符数组
        let mut descs = rx_ring.desc.as_desc_slice();

        // 遍历所有待处理的接收描述符
        while descs[rdt].status & E1000_RXD_STAT_DD as u8 != 0 {
            // 获取数据包长度
            let packet_len = descs[rdt].length as usize;
            // 获取缓冲区中的 SKB（socket buffer）
            let buf = &mut rx_ring.buf.borrow_mut();
            let skb = &buf[rdt].as_mut().unwrap().1;

            // 将接收到的数据填入 SKB
            skb.put(packet_len as u32);
            // 识别协议类型并设置到 SKB 中
            let protocol = skb.eth_type_trans(dev);
            skb.protocol_set(protocol);

            // 将 SKB 交给 NAPI 进行处理
            data.napi.gro_receive(skb);

            // 为下一个接收描述符分配新的 SKB
            let skb_new = dev.alloc_skb_ip_align(RXTX_SINGLE_RING_BLOCK_SIZE as u32).unwrap();
            let dma_map = dma::MapSingle::try_new(&*data.dev, skb_new.head_data().as_ptr() as *mut u8, RXTX_SINGLE_RING_BLOCK_SIZE, bindings::dma_data_direction_DMA_FROM_DEVICE).unwrap();
            descs[rdt].buf_addr = dma_map.dma_handle as u64;
            buf[rdt] = Some((dma_map, skb_new));

            // 清除当前描述符的状态，并更新接收队列的尾部索引
            descs[rdt].status = 0;
            data.e1000_hw_ops.e1000_write_rx_queue_tail(rdt as u32);
            rdt = (rdt + 1) % RX_RING_SIZE;
        }

        // 回收传输队列中的资源
        NetDevice::e1000_recycle_tx_queue(dev, data);
        // 完成 NAPI 的处理
        data.napi.complete_done(1);
        // 返回处理的包数
        1
    }
}

// 定义 E1000Drv 结构体用于 PCI 驱动的实现
struct E1000Drv {}

impl pci::Driver for E1000Drv {
    // `Data` 类型表示驱动程序私有数据的包装，使用 `Box<E1000DrvPrvData>` 类型
    type Data = Box<E1000DrvPrvData>;

    // 定义 PCI 设备 ID 表
    kernel::define_pci_id_table! {(), [
        (pci::DeviceId::new(E1000_VENDER_ID, E1000_DEVICE_ID), None),
    ]}

    // 设备探测函数，用于初始化和配置 PCI 设备
    fn probe(dev: &mut pci::Device, id: core::option::Option<&Self::IdInfo>) -> Result<Self::Data> {
        pr_info!("Rust for linux e1000 driver demo (probe): {:?}\n", id);

        // 注意：目前只支持 QEMU 的 82540EM 芯片。

        // 选择 PCI 设备的 BAR（基址寄存器），根据指定的条件筛选出需要的资源
        let bars = dev.select_bars((bindings::IORESOURCE_MEM | bindings::IORESOURCE_IO) as u64);

        // 启用 PCI 设备
        dev.enable_device()?;

        // 请求所选 BAR 的物理内存区域
        dev.request_selected_regions(bars, c_str!("e1000 reserved memory"))?;

        // 设置设备为主模式
        dev.set_master();

        // 获取由 BAR0 提供的资源（内存区域）
        let mem_res = dev.iter_resource().next().ok_or(kernel::error::code::EIO)?;
        // 获取 I/O 端口地址
        let io_res = dev.iter_resource().skip(1).find(|r:&Resource|r.check_flags(bindings::IORESOURCE_IO)).ok_or(kernel::error::code::EIO)?;

        // TODO: `pci_save_state` 函数暂时不支持，只能使用原始的 C 绑定

        // 分配新的以太网设备，相当于 C 版本中的 `alloc_etherdev()` 和 `SET_NETDEV_DEV()`
        let mut netdev_reg = net::Registration::<NetDevice>::try_new(dev)?;
        let netdev = netdev_reg.dev_get();

        // 将设备寄存器的硬件地址映射到逻辑地址，以便内核驱动可以访问
        let mem_addr = Arc::try_new(dev.map_resource(&mem_res, mem_res.len())?)?;
        let io_addr = Arc::try_new(pci::IoPort::try_new(&io_res)?)?;

        // TODO: 实现 C 版本中的 `e1000_init_hw_struct()`

        // 只针对 PCI-X 需要 64 位，为简化代码，这里硬编码为 32 位
        dma::set_coherent_mask(dev, 0xFFFFFFFF)?;

        // TODO: 这里实现 ethtool 支持

        // 启用 NAPI，R4L 将调用 `netif_napi_add_weight()`，而原始 C 版本调用 `netif_napi_add`
        let napi = net::NapiAdapter::<NapiHandler>::add_weight(&netdev, 64)?;

        // TODO: 实现 C 版本中的 `e1000_sw_init()`

        // TODO: 许多功能标志在 C 代码中进行分配，这里暂时跳过
        let e1000_hw_ops = E1000Ops {
            mem_addr: Arc::clone(&mem_addr),
            io_addr: Arc::clone(&io_addr),
        };
        e1000_hw_ops.e1000_reset_hw()?;

        // TODO: 目前硬编码 MAC 地址，应该从 EEPROM 中读取
        netdev.eth_hw_addr_set(&MAC_HWADDR);

        // TODO: 背景任务和 Wake on LAN 目前不支持

        // 获取中断号
        let irq = dev.irq();

        // 从设备获取通用设备
        let common_dev = device::Device::from_dev(dev);

        // 关闭网络设备的 carrier 状态
        netdev.netif_carrier_off();

        // SAFETY: `spinlock_init` 在下方被调用
        let mut tx_ring = unsafe { SpinLock::new(None) };
        let mut rx_ring = unsafe { SpinLock::new(None) };
        // SAFETY: 我们不会移动 `tx_ring` 和 `rx_ring`
        kernel::spinlock_init!(unsafe { Pin::new_unchecked(&mut tx_ring) }, "tx_ring");
        kernel::spinlock_init!(unsafe { Pin::new_unchecked(&mut rx_ring) }, "rx_ring");

        unsafe {
            let pci_dev = dev.get_pci_device_ptr();

            // 注册网络设备及其私有数据
            netdev_reg.register(Box::try_new(
                NetDevicePrvData {
                    dev: Arc::try_new(common_dev)?,
                    e1000_hw_ops: Arc::try_new(e1000_hw_ops)?,
                    napi: napi.into(),
                    tx_ring,
                    rx_ring,
                    irq,
                    _irq_handler: AtomicPtr::new(core::ptr::null_mut()),
                    pci_dev: Arc::try_new(pci_dev)?,
                }
            )?)?;

            // 返回驱动程序私有数据
            Ok(Box::try_new(
                E1000DrvPrvData {
                    // 必须持有这个注册，否则设备将被移除
                    _netdev_reg: netdev_reg,
                }
            )?)
        }
    }

    // 设备移除函数
    fn remove(data: &Self::Data) {
        pr_info!("Rust for linux e1000 driver demo (remove)\n");

        // 获取私有数据
        let edpd = data.as_ref(); // 驱动程序私有数据
        let dev = &*(edpd._netdev_reg.dev_get());  // 转换为 &Device
        let dev_ptr = unsafe{ dev.get_net_device_ptr()};  // 获取 net_device 指针
        let drvdata = unsafe { &*(bindings::dev_get_drvdata(&mut (*dev_ptr).dev) as *const NetDevicePrvData) }; // 获取 Box<NetDevicePrvData>
        let pci_dev = unsafe { drvdata.pci_dev.as_ref() };  // 获取 pci_dev: *mut bindings::pci_dev

        // 注销中断处理程序
        let irq_handler_ptr = drvdata._irq_handler.load(core::sync::atomic::Ordering::Relaxed);
        if !irq_handler_ptr.is_null() {
            unsafe { Box::from_raw(irq_handler_ptr) };
        }

        // 释放 PCI 设备资源
        let bars = unsafe { bindings::pci_select_bars(*pci_dev, (bindings::IORESOURCE_MEM | bindings::IORESOURCE_IO) as u64) } as i32;
        unsafe { bindings::pci_release_selected_regions(*pci_dev, bars) };
    }

}

// 定义 E1000KernelMod 结构体，用于内核模块管理
struct E1000KernelMod {
    // `Pin<Box<driver::Registration::<pci::Adapter<E1000Drv>>>>` 表示驱动注册对象，必须固定在内存中以避免驱动被意外移除
    _dev: Pin<Box<driver::Registration::<pci::Adapter<E1000Drv>>>>,
}

// 实现 kernel::Module 特征以支持内核模块操作
impl kernel::Module for E1000KernelMod {
    // 模块初始化函数，`name` 是模块名称，`module` 是当前模块的引用
    fn init(name: &'static CStr, module: &'static ThisModule) -> Result<Self> {
        // 打印初始化日志
        pr_info!("Rust for linux e1000 driver demo (init)\n");

        // 使用驱动注册对象 `driver::Registration` 来注册 PCI 驱动
        let d = driver::Registration::<pci::Adapter<E1000Drv>>::new_pinned(name, module)?;

        // 将驱动注册对象存储在模块结构体中，否则它会被丢弃，从而导致驱动被移除
        Ok(E1000KernelMod {_dev: d})
    }
}

// 实现 Drop 特征以处理模块卸载时的清理操作
impl Drop for E1000KernelMod {
    // 在模块卸载时打印日志
    fn drop(&mut self) {
        pr_info!("Rust for linux e1000 driver demo (exit)\n");
    }
}

