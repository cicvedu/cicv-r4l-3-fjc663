use kernel::prelude::*;
use kernel::pci::{MappedResource, IoPort};
use kernel::delay::coarse_sleep;
use kernel::sync::Arc;

use core::time::Duration;

use crate::ring_buf::{RxRingBuf, TxRingBuf};

use crate::consts::*;

pub(crate) struct E1000Ops {
    pub(crate) mem_addr: Arc<MappedResource>, // 内存映射资源的引用
    pub(crate) io_addr: Arc<IoPort>, // I/O 端口的引用
}

impl E1000Ops {

    /// 完全重置硬件，对应于 C 版本的 `e1000_reset_hw`。
    /// 仅支持 QEMU 的 82540EM 芯片。
    pub(crate) fn e1000_reset_hw(&self) -> Result {
        // 清除中断掩码寄存器，以停止板卡生成任何中断
        // 这确保在重置过程中不会受到中断干扰
        self.mem_addr.writel(0xffffffff, E1000_IMC)?;

        // 禁用接收控制寄存器 (RCTL) 和传输控制寄存器 (TCTL)
        // 允许任何待处理的事务在进行全局重置之前完成
        self.mem_addr.writel(0, E1000_RCTL)?;
        self.mem_addr.writel(E1000_TCTL_PSP, E1000_TCTL)?;

        // 刷新写缓冲区，以确保写入寄存器的操作完成
        self.e1000_write_flush();

        // 延迟 10 毫秒，以允许任何未完成的 PCI 事务完成
        coarse_sleep(Duration::from_millis(10));

        // 读取当前控制寄存器的值
        let ctrl = self.mem_addr.readl(E1000_CTRL)?;

        // 使用 I/O 映射发出重置操作，因为这些控制器无法在发出 64 位写操作时进行确认
        self.e1000_write_reg_io(ctrl | E1000_CTRL_RST, E1000_CTRL)?;

        // 在 MAC 重置后，强制重新加载 EEPROM，以恢复设备的上电设置
        // 对于较新的控制器，EEPROM 会自动重新加载
        coarse_sleep(Duration::from_millis(5));

        // 在启用了 ASF（高级安全功能）的适配器上禁用硬件 ARP
        // 这可能会影响 ARP 请求的处理
        let manc = self.mem_addr.readl(E1000_MANC)?;
        self.mem_addr.writel(manc & (!E1000_MANC_ARP_EN), E1000_MANC)?;

        // 清除中断掩码寄存器，以停止板卡生成任何中断
        self.mem_addr.writel(0xffffffff, E1000_IMC)?;

        // 读取并清除中断状态寄存器，以确保没有挂起的中断事件
        self.mem_addr.readl(E1000_ICR)?;

        Ok(())
    }

    // 写入并刷新寄存器以确保操作完成
    fn e1000_write_flush(&self) {
        // 读取状态寄存器，该操作应该不会失败
        self.mem_addr.readl(E1000_STATUS).unwrap();
    }

    // 通过 I/O 端口写入寄存器
    fn e1000_write_reg_io(&self, value: u32, addr: usize) -> Result {
        // 写入地址和数据到 I/O 端口
        self.io_addr.outl(addr as u32, 0)?;
        self.io_addr.outl(value, 4)?;
        Ok(())
    }

    // 配置接收和发送缓冲区以及相关中断
    pub(crate) fn e1000_configure(&self, rx_ring: &RxRingBuf, tx_ring: &TxRingBuf) -> Result {
        // 配置接收缓冲区
        self.e1000_configure_rx(rx_ring)?;
        // 配置发送缓冲区
        self.e1000_configure_tx(tx_ring)?;

        // 启用相关中断
        self.mem_addr.writel(
            E1000_ICR_TXDW | E1000_ICR_RXT0 | E1000_ICR_RXDMT0 | E1000_ICR_RXSEQ | E1000_ICR_LSC,
            E1000_IMS
        )?;
        Ok(())
    }

    // 配置发送缓冲区
    fn e1000_configure_tx(&self, tx_ring: &TxRingBuf) -> Result {
        // 根据手册第 14.5 节配置发送缓冲区

        // 设置发送缓冲区的头索引、尾索引和缓冲区大小
        self.mem_addr.writel(0, E1000_TDH)?; // 设置头索引
        self.mem_addr.writel(0, E1000_TDT)?; // 设置尾索引
        self.mem_addr.writel((TX_RING_SIZE * 16) as u32, E1000_TDLEN)?; // 设置缓冲区长度
        // 设置发送缓冲区的起始地址
        self.mem_addr.writel(tx_ring.desc.get_dma_addr() as u32, E1000_TDBAL)?;
        self.mem_addr.writel(0, E1000_TDBAH)?;

        // 配置发送控制寄存器
        let tctl = (
            E1000_TCTL_EN | // 启用发送单元
                E1000_TCTL_PSP | // 填充发送包
                0x10 << E1000_CT_SHIFT | // 设置计时器
                0x40 << E1000_COLD_SHIFT // 设置冷却时间
        );
        self.mem_addr.writel(tctl, E1000_TCTL)?;

        // 配置发送间隔寄存器
        let tipg = (
            DEFAULT_82543_TIPG_IPGT_COPPER | // 设置 IPGT
                DEFAULT_82543_TIPG_IPGR1 << E1000_TIPG_IPGR1_SHIFT | // 设置 IPGR1
                DEFAULT_82543_TIPG_IPGR2 << E1000_TIPG_IPGR2_SHIFT // 设置 IPGR2
        );
        self.mem_addr.writel(tipg, E1000_TIPG)?;

        Ok(())
    }

    // 配置接收缓冲区
    fn e1000_configure_rx(&self, rx_ring: &RxRingBuf) -> Result {
        // 根据手册第 14.4 节配置接收缓冲区

        // 根据 MIT6.828 练习 10，硬编码 QEMU 的 MAC 地址
        // MAC 地址：52:54:00:12:34:56
        self.mem_addr.writel(0x12005452, E1000_RA)?; // 设置 RAL
        self.mem_addr.writel(0x5634 | (1 << 31), E1000_RA + 4)?; // 设置 RAH

        // 清除多播地址表中的所有条目
        for i in 0..128 {
            self.mem_addr.writel(0, E1000_MTA + i * 4)?;
        }

        // 配置接收缓冲区的头索引、尾索引和缓冲区大小
        self.mem_addr.writel(0, E1000_RDH)?; // 设置头索引
        self.mem_addr.writel((RX_RING_SIZE - 1) as u32, E1000_RDT)?; // 设置尾索引
        self.mem_addr.writel((RX_RING_SIZE * 16) as u32, E1000_RDLEN)?; // 设置缓冲区长度
        // 设置接收缓冲区的起始地址
        self.mem_addr.writel(rx_ring.desc.get_dma_addr() as u32, E1000_RDBAL)?;
        self.mem_addr.writel(0, E1000_RDBAH)?;

        // 配置接收控制寄存器
        let rctl = (
            E1000_RCTL_EN | // 启用接收单元
                E1000_RCTL_BAM | // 启用广播接收
                E1000_RCTL_SZ_2048 | // 设置接收缓冲区大小
                E1000_RCTL_SECRC // 启用硬件 CRC 校验
        );
        self.mem_addr.writel(rctl, E1000_RCTL)?;

        // 禁用 RDTR 和 RADV 计时器，因为我们使用 NAPI，不需要硬件帮助来减少中断
        self.mem_addr.writel(0, E1000_RDTR)?;
        self.mem_addr.writel(0, E1000_RADV)?;

        Ok(())
    }

    // 读取中断状态寄存器的值
    pub(crate) fn e1000_read_interrupt_state(&self) -> u32 {
        self.mem_addr.readl(E1000_ICR).unwrap()
    }

    // 读取发送队列头索引
    pub(crate) fn e1000_read_tx_queue_head(&self) -> u32 {
        self.mem_addr.readl(E1000_TDH).unwrap()
    }

    // 读取发送队列尾索引
    pub(crate) fn e1000_read_tx_queue_tail(&self) -> u32 {
        self.mem_addr.readl(E1000_TDT).unwrap()
    }

    pub(crate) fn e1000_write_tx_queue_tail(&self, val: u32) {
        self.mem_addr.writel(val, E1000_TDT).unwrap()
    }


    pub(crate) fn e1000_read_rx_queue_head(&self) -> u32 {
        self.mem_addr.readl(E1000_RDH).unwrap()
    }

    pub(crate) fn e1000_read_rx_queue_tail(&self) -> u32 {
        self.mem_addr.readl(E1000_RDT).unwrap()
    }

    pub(crate) fn e1000_write_rx_queue_tail(&self, val: u32) {
        self.mem_addr.writel(val, E1000_RDT).unwrap()
    }


}

