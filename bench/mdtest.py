import matplotlib
import matplotlib.pyplot as plt
import re


def get_data() -> dict:
    mdtest_files = ["dbfs.txt", "ext4.txt", "ext3.txt"]
    data = {}
    for mdtest_file in mdtest_files:
        with open("./result/mdtest/" + mdtest_file, "r") as f:
            # 定义正则表达式，用于匹配操作类型和操作结果
            pattern = re.compile(r"^\s*(\w+\s*\w*)\s+(\d+\.\d+)\s+(\d+\.\d+)\s+(\d+\.\d+)\s+(\d+\.\d+)")
            # 用正则表达式匹配出操作类型和操作结果
            results = {operation: {"max": 0, "min": 0, "mean": 0, "std_dev": 0} for operation in
                       ["Directory creation", "Directory stat", "Directory rename", "Directory removal",
                        "File creation",
                        "File stat", "File read", "File removal", "Tree creation", "Tree removal"]}
            for line in f.readlines():
                match = pattern.match(line)
                if match:
                    operation = match.group(1)
                    results[operation]["max"] = float(match.group(2))
                    results[operation]["min"] = float(match.group(3))
                    results[operation]["mean"] = float(match.group(4))
                    results[operation]["std_dev"] = float(match.group(5))
            data[mdtest_file] = results
            # 打印文件名
            # print(mdtest_file +" :")
            # 打印结果
            # for operation in results:
            #     print(operation, results[operation])
    return data


def draw_mdtest(data: dict):
    # 只关注mean
    # 绘制柱状图，横坐标为操作类型，纵坐标为操作结果，每个操作类型对应一列，每列有四个柱子，分别代表四种文件系统

    # 操作类型
    operations = ["Directory creation", "Directory stat", "Directory rename", "Directory removal", "File creation",
                  "File stat", "File read", "File removal", "Tree creation", "Tree removal"]
    # 操作结果
    dbfs_means = [data["dbfs.txt"][operation]["mean"] for operation in operations]
    ext4_means = [data["ext4.txt"][operation]["mean"] for operation in operations]
    ext2_means = [data["ext3.txt"][operation]["mean"] for operation in operations]

    # 以ext4为基准，计算其他文件系统的相对性能
    dbfs_means = [dbfs_means[i] / ext4_means[i] for i in range(len(dbfs_means))]
    ext2_means = [ext2_means[i] / ext4_means[i] for i in range(len(ext2_means))]
    ext4_means = [1.0 for i in range(len(ext4_means))]

    # max value
    max_value = max([max(dbfs_means), max(ext4_means), max(ext2_means)])
    # max_value = max_value + max_value * 0.1
    # 柱状图宽度
    width = 0.2
    # 柱状图间距
    x = [i for i in range(len(operations))]
    x1 = x
    x2 = [i + width for i in x]
    x3 = [i + width * 2 for i in x]

    # 设置图像大小
    plt.figure(figsize=(8, 6))
    plt.yscale('log')
    # 绘制柱状图
    plt.bar(x1, dbfs_means, width=width, label="dbfs",color=rgb2hex(38, 70, 83))
    plt.bar(x2, ext4_means, width=width, label="ext4",color=rgb2hex(230, 111, 81))
    plt.bar(x3, ext2_means, width=width, label="ext3",color=rgb2hex (233, 196, 107))

    # 设置横坐标
    operations = ["Dir\ncreate", "Dir\nstat", "Dir\nrename", "Dir\nremove", "File\ncreate",
                  "File\nstat", "File\nread", "File\nremove", "Tree\ncreate", "Tree\nremove"]
    plt.xticks(x2, operations)

    # 调整字体居中对齐
    plt.xticks(fontsize=8, ha='center')

    plt.ylabel("Relative performance")
    # 设置纵坐标

    # 设置标题
    plt.title("mdtest performance")
    # 设置图例
    plt.legend()
    # save svg
    plt.savefig("./result/mdtest/mdtest.svg")
    # 显示图像
    plt.show()
def rgb2hex(r: int, g: int, b: int) -> str:
    return '#%02x%02x%02x' % (r, g, b)


if __name__ == '__main__':
    data = get_data()
    draw_mdtest(data)
