## 注册字符设备

### 1. 修改配置
```shell
make LLVM=1 menuconfig
```
![](/images/img5_1.png)
重新编译
```shell
make LLVM=1 -j$(nproc)
```

### 2. 修改samples/rust/rust_chrdev.rs
写入字符设备
```rust
fn write(_this: &Self, _file: &file::File, _reader: &mut impl kernel::io_buffer::IoBufferReader, _offset: u64) -> Result<usize> {
        let mut guard = _this.inner.lock(); // 锁定全局内存缓冲区
        let buffer = &mut *guard;

        // 检查偏移量是否超出缓冲区大小
        if _offset as usize >= GLOBALMEM_SIZE {
            return Err(EINVAL); // 返回无效参数错误
        }

        // 计算剩余空间
        let remaining_space = GLOBALMEM_SIZE - _offset as usize;
        // 计算实际要写入的数据大小，不能超过reader中的数据长度和剩余空间
        let data_to_write = core::cmp::min(_reader.len(), remaining_space);

        // 将数据从reader读取到全局内存缓冲区中
        unsafe {
            _reader.read_raw(buffer.as_mut_ptr().add(_offset as usize), data_to_write)?;
        }

        Ok(data_to_write) // 返回实际写入的数据大小
    }
```
读出字符设备
```rust
fn read(_this: &Self, _file: &file::File, _writer: &mut impl kernel::io_buffer::IoBufferWriter, _offset: u64) -> Result<usize> {
        let guard = _this.inner.lock(); // 锁定全局内存缓冲区
        let buffer = &*guard;

        // 检查偏移量是否超出缓冲区大小
        if _offset as usize >= GLOBALMEM_SIZE {
            return Ok(0); // 超出缓冲区大小，返回EOF
        }

        // 计算剩余数据量
        let remaining_data = GLOBALMEM_SIZE - _offset as usize;
        // 计算实际要读取的数据大小，不能超过writer中的可写入长度和剩余数据量
        let data_to_read = core::cmp::min(_writer.len(), remaining_data);

        // 将数据从全局内存缓冲区读取到writer中
        unsafe {
            _writer.write_raw(buffer.as_ptr().add(_offset as usize), data_to_read)?;
        }

        Ok(data_to_read) // 返回实际读取的数据大小
    }
```
### 3. 测试结果
![](/images/img5_2.png)

### 4.问题回答
Q：作业5中的字符设备/dev/cicv是怎么创建的？它的设备号是多少？它是如何与我们写的字符设备驱动关联上的？
> - 在build_image.sh脚本里有一条命令`echo "mknod /dev/cicv c 248 0" >> etc/init.d/rcS`，
    这表明是在系统初始时使用***mknod***命令创建的/dev/cicv
> - 主设备号：248，次设备号：0
> - 字符设备驱动根据主设备号与字符设备关联，主设备号 248 对应的字符设备驱动为***rust_chrdev***