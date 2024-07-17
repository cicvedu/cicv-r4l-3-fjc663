## 作业1：编译Linux内核

进入Linux文件夹，使用如下命令进行编译：

### 1. 生成默认配置文件
这条命令的作用是生成适用于x86_64架构的默认内核配置文件
- make：调用GNU Make工具，它使用当前目录下的Makefile来执行任务。
- x86_64_defconfig：这是一个预定义的配置文件，适用于x86_64架构。它会在当前目录下生成一个名为.config的配置文件，该文件包含内核的配置选项。
```bash
make x86_64_defconfig
```
![](/images/img1_1.png)

### 2. 交互式修改内核配置
这条命令用于在生成的默认配置基础上进一步定制内核配置。具体步骤如下：
- LLVM=1：告诉Make使用LLVM编译器工具链（如Clang）而不是默认的GCC。
- menuconfig：启动一个基于菜单的用户界面，允许用户交互式地修改内核配置选项。

在menuconfig界面中，导航到“General setup”部分，并启用“Rust support”选项。以下是操作步骤：
- 使用方向键导航到“General setup”。
- 按回车键进入子菜单。
- 查找并启用“Rust support”选项（按空格键选择）。
```bash
make LLVM=1 menuconfig
```
![](/images/img1_2.png)

### 3. 并行编译内核
- LLVM=1：继续使用LLVM编译器工具链。
- -j$(nproc)：-j选项用于并行编译，$(nproc)命令返回当前系统的处理器核心数，以便最大化利用CPU资源，加速编译过程。
```bash
make LLVM=1 -j$(nproc)
```
编译成功后，在Linux文件夹下可以看到一个名为vmlinux的文件，这个文件是编译生成的Linux内核镜像。