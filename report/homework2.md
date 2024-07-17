## 作业2：对Linux内核进行一些配置

1. 编译成内核模块，是在哪个文件中以哪条语句定义的？
   - Kconfig定义的配置项，在配置完成后，会写入.config文件
   - config文件中存储的tristate类型配置项
2. 该模块位于独立的文件夹内，却能编译成Linux内核模块，这叫做out-of-tree module，请分析它是如何与内核代码产生联系的？
   - 编写Kbuild Makefile，定义编译目标和涉及到的文件
   - 使用obj-m编译成模块

### 1. 安装cpio
```shell
apt-get update
apt-get install cpio
```
这里cpio 的作用是将根文件系统的内容打包成一个initramfs镜像文件。这个镜像文件将在内核启动时被加载，
并提供必要的文件系统环境，以便系统可以完成引导过程。

### 2. 禁用默认的e1000网卡驱动
- 默认情况下，Linux内核启用了e1000网卡驱动（通常是针对Intel PRO/1000 Gigabit以太网卡的驱动）。要禁用这个默认驱动，需要修改内核配置。
- 具体路径是：Device Drivers > Network device support > Ethernet driver support > Intel devices, Intel(R) PRO/1000 Gigabit Ethernet support。
- 在这个配置路径下，禁用e1000驱动，以便系统不再使用默认的e1000驱动。
  ![](/images/img2_2.png)

### 3. 运行build_image.sh脚本
首先进入src_e1000文件夹，运行build_image.sh脚本来生成一个包含新编译内核的磁盘镜像：
```shell
./build_image.sh
```

### 4. 配置网卡驱动模块
```shell
insmod r4l_e1000_demo.ko
ip link set eth0 up
ip addr add broadcast 10.0.2.255 dev eth0
ip addr add 10.0.2.15/255.255.255.0 dev eth0
ip route add default via 10.0.2.1
ping 10.0.2.2
```
1. 使用 insmod 命令将 r4l_e1000_demo.ko 内核模块加载到当前正在运行的内核中。
   ![](/images/img2_1.png)
2. 启用名为 eth0 的网络接口。
   ![](/images/img2_4.png)
3. 为 eth0 网络接口配置广播地址。
4. 为 eth0 网络接口配置静态 IP 地址和子网掩码。
5. 添加一个默认路由，使得所有不在本地网络内的数据包都通过指定的网关发送。
6. 使用 ping 命令测试与目标 IP 地址 10.0.2.2 的网络连接。
![](/images/img2_3.png)


