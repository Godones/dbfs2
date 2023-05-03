import matplotlib.pyplot as plt

output_path = "./result/mdtest-opt/mdtest-opt.svg"


def draw(data1: list, data2: list, data3: list):
    operations = ["Directory creation", "Directory stat", "Directory rename", "Directory removal", "File creation",
                  "File stat", "File read", "File removal", "Tree creation", "Tree removal"]
    # 绘制堆叠柱状图
    plt.bar(operations, data1, width=0.2, label="dbfs")
    plt.bar(operations, data2, width=0.2, label="ext4")
    plt.bar(operations, data3, width=0.2, label="ext2")
    # 设置横坐标
    plt.xticks(operations, operations)
    # 调整字体居中对齐
    plt.xticks(fontsize=8, ha='center')
    # 设置纵坐标并显示大小
    plt.ylim(0, 100)
    # 设置标题
    plt.title("mdtest")
    # 设置图例
    plt.legend()
    # save svg
    plt.savefig(output_path)
    # 显示图像
    # plt.show()


if __name__ == '__main__':
    data1 = [11191 + 8885 + 9085, 267489 + 247315 + 253307, 10366 + 8413 + 8662, 10582 + 10212 + 10768,
             8367 + 7741 + 7938, 262001 + 269992 + 246260, 23045 + 19123 + 37285,
             9165 + 9774 + 8257, 6873 + 6787 + 4475, 12543 + 10297 + 11748]
    data2 = [11171 + 10460 + 8644, 226292 + 267688 + 237270, 9758 + 9369 + 8407, 10840 + 9722 + 10788,
             8969 + 8031 + 8007,
             231930 + 286904 + 191765, 20235 + 22072 + 33208, 9776 + 13406 + 8011, 8136 + 7682 + 9063,
             12007 + 12185 + 10063]
    data3 = []

    draw(data1, data2, data3)
