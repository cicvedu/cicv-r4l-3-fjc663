use kernel::net::SkBuff;
use kernel::prelude::*;
use kernel::dma;
use core::cell::RefCell;
use crate::hw_defs::{RxDescEntry, TxDescEntry};

/// 一个由 SkBuff 和其 DMA 映射组成的元组
pub(crate) type SkbDma = (dma::MapSingle::<u8>, ARef<SkBuff>);

/// 对 `dma::Allocation` 的切片视图
pub(crate) struct DmaAllocSlice<T> {
    desc: dma::Allocation::<T>,  // DMA 分配的描述符
    count: usize,  // 描述符的数量
}

impl<T> DmaAllocSlice<T> {
    /// 返回描述符的可变切片视图
    pub(crate) fn as_desc_slice(&mut self) -> &mut [T] {
        // 使用不安全代码从原始指针创建可变切片
        unsafe { core::slice::from_raw_parts_mut(self.desc.cpu_addr, self.count) }
    }

    /// 获取 DMA 地址
    pub(crate) fn get_dma_addr(&self) -> usize {
        self.desc.dma_handle as usize
    }

    /// 获取 CPU 地址
    pub(crate) fn get_cpu_addr(&self) -> usize {
        self.desc.cpu_addr as usize
    }
}

/// 环形缓冲区结构体
pub(crate) struct RingBuf<T> {
    pub(crate) desc: DmaAllocSlice<T>,  // DMA 描述符的切片视图
    pub(crate) buf: RefCell<Vec<Option<SkbDma>>>,  // 包含 SkbDma 的可变缓冲区
    pub(crate) next_to_clean: usize,  // 下一个要清理的描述符索引
}

impl<T> RingBuf<T> {
    /// 创建一个新的环形缓冲区
    pub(crate) fn new(desc: dma::Allocation::<T>, len: usize) -> Self {
        // 创建一个新的可变缓冲区
        let buf = RefCell::new(Vec::new());

        // 初始化缓冲区，填充 None
        {
            let mut buf_ref = buf.borrow_mut();
            for _ in 0..len {
                buf_ref.try_push(None).unwrap();
            }
        }

        // 创建 DMA 描述符的切片视图
        let desc = DmaAllocSlice {
            desc,
            count: len,
        };

        // 返回新的环形缓冲区实例
        Self { desc, buf, next_to_clean: 0 }
    }
}

// 为接收描述符定义类型别名
pub(crate) type RxRingBuf = RingBuf<RxDescEntry>;
// 为发送描述符定义类型别名
pub(crate) type TxRingBuf = RingBuf<TxDescEntry>;
