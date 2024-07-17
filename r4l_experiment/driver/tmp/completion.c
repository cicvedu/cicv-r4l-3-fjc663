#include <linux/module.h>
#include <linux/init.h> 
#include <linux/sched.h>
#include <linux/kernel.h>
#include <linux/fs.h>
#include <linux/cdev.h>
#include <linux/types.h>
#include <linux/completion.h>

#include "completion.h" // 包含自定义completion设备的头文件

static int completion_major = 0, completion_minor = 0; // 设备的主设备号和次设备号

static struct completion_dev completion_dev; // 定义completion设备的结构体

// 打开设备文件的操作
static int completion_open(struct inode *inode, struct file *filp)
{
    pr_info("%s() is invoked\n", __FUNCTION__); // 打印日志，表示函数被调用

    // 将inode中的cdev成员指向的结构体转换为completion_dev结构体，并存储在file的private_data中
    filp->private_data = container_of(inode->i_cdev, struct completion_dev, cdev);

    return 0; // 返回0表示成功
}

// 从设备读取数据的操作
static ssize_t completion_read(struct file *filp, char __user *buf, size_t count, loff_t *pos)
{
    struct completion_dev *dev = filp->private_data; // 获取设备的私有数据

    pr_info("%s() is invoked\n", __FUNCTION__); // 打印日志，表示函数被调用

    // 打印日志，当前进程将进入睡眠状态
    pr_info("process %d(%s) is going to sleep\n", current->pid, current->comm);
    wait_for_completion(&dev->completion); // 等待完成事件
    // 打印日志，当前进程被唤醒
    pr_info("awoken %d(%s)\n", current->pid, current->comm);

    return 0; // 返回0表示读取0字节
}

// 向设备写入数据的操作
static ssize_t completion_write(struct file *filp, const char __user *buf, size_t count, loff_t *pos)
{
    struct completion_dev *dev = filp->private_data; // 获取设备的私有数据

    pr_info("%s() is invoked\n", __FUNCTION__); // 打印日志，表示函数被调用

    // 打印日志，当前进程将唤醒所有等待的进程
    pr_info("process %d(%s) awakening the readers...\n", current->pid, current->comm);
    complete(&dev->completion); // 完成事件，唤醒等待的进程

    return count; // 返回写入的字节数
}

// 文件操作结构体，定义了设备文件的各种操作
static struct file_operations completion_fops = {
    .owner = THIS_MODULE,
    .open  = completion_open,
    .read  = completion_read,
    .write = completion_write,
};

// 模块初始化函数
static int __init m_init(void)
{
    int err = 0; // 错误码
    dev_t devno; // 设备号

    printk(KERN_WARNING MODULE_NAME " is loaded\n"); // 打印日志，表示模块已加载

    init_completion(&completion_dev.completion); // 初始化completion结构体

    // 分配字符设备区域
    err = alloc_chrdev_region(&devno, completion_minor, 1, MODULE_NAME);
    if (err < 0) {
        pr_info("Cant't get major"); // 打印日志，表示无法获取主设备号
        return err; // 返回错误码
    }
    completion_major = MAJOR(devno); // 获取主设备号

    cdev_init(&completion_dev.cdev, &completion_fops); // 初始化字符设备结构体

    devno = MKDEV(completion_major, completion_minor); // 创建设备号
    err = cdev_add(&completion_dev.cdev, devno, 1); // 将字符设备添加到系统中
    if (err) {
        pr_info("Error(%d): Adding completion device error\n", err); // 打印日志，表示添加设备失败
        return err; // 返回错误码
    }

    return 0; // 返回0表示成功
}

// 模块退出函数
static void __exit m_exit(void)
{
    dev_t devno; // 设备号

    printk(KERN_WARNING MODULE_NAME " unloaded\n"); // 打印日志，表示模块已卸载

    cdev_del(&completion_dev.cdev); // 删除字符设备

    devno = MKDEV(completion_major, completion_minor); // 创建设备号
    unregister_chrdev_region(devno, 1); // 注销设备号区域
}

module_init(m_init); // 指定模块初始化函数
module_exit(m_exit); // 指定模块退出函数

MODULE_LICENSE("GPL"); // 指定模块的许可证
MODULE_AUTHOR("Tester"); // 指定模块的作者
MODULE_DESCRIPTION("Example of Kernel's completion mechanism"); // 指定模块的描述