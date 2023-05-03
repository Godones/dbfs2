import matplotlib.pyplot as plt

output_path = "./result/mdtest-opt/mdtest-opt.svg"


def draw(data1: list, data2: list, data3: list):
    operations = ["Dir\ncreate", "Dir\nstat", "Dir\nrename", "Dir\nremove", "File\ncreate",
                  "File\nstat", "File\nread", "File\nremove", "Tree\ncreate", "Tree\nremove"]

    max_value = max([max(data1), max(data2), max(data3)])

    plt.figure(figsize=(8, 6))

    width = 0.2
    # 柱状图间距
    x = [i for i in range(len(operations))]
    x1 = x
    x2 = [i + width for i in x]
    x3 = [i + width * 2 for i in x]

    plt.bar(x1, data1, width=width, label="opt1")
    plt.bar(x2, data2, width=width, label="opt2")
    plt.bar(x3, data3, width=width, label="opt3")

    # 设置横坐标
    operations = ["Dir\ncreate", "Dir\nstat", "Dir\nrename", "Dir\nremove", "File\ncreate",
                  "File\nstat", "File\nread", "File\nremove", "Tree\ncreate", "Tree\nremove"]
    plt.xticks(x2, operations)
    # 调整字体居中对齐
    plt.xticks(fontsize=8, ha='center')
    # 设置纵坐标并显示大小
    plt.ylim(0, max_value)
    # 设置标题
    plt.title("mdtest")
    # 设置图例
    plt.legend()
    # save svg
    plt.savefig(output_path)
    # 显示图像
    # plt.show()


if __name__ == '__main__':
    data1 = [11191 + 8885 + 9085,
             267489 + 247315 + 253307,
             10366 + 8413 + 8662,
             10582 + 10212 + 10768,
             8367 + 7741 + 7938,
             262001 + 269992 + 246260,
             23045 + 19123 + 37285,
             9165 + 9774 + 8257,
             6873 + 6787 + 4475,
             12543 + 10297 + 11748]

    data2 = [11171 + 10460 + 8644,
             226292 + 267688 + 237270,
             9758 + 9369 + 8407,
             10840 + 9722 + 10788,
             8969 + 8031 + 8007,
             231930 + 286904 + 191765,
             20235 + 22072 + 33208,
             9776 + 13406 + 8011,
             8136 + 7682 + 9063,
             12007 + 12185 + 10063]
    data3 = [
        12868 + 10848 + 10258,
        245506 + 314961 + 308233,
        11337 + 11112 + 11680,
        15886 + 15714 + 15979,
        10405 + 10492 + 15979,
        346753 + 342310 + 336248,
        48593 + 48321 + 46808,
        15805 + 16073 + 16569,
        12345 + 9345 + 8736,
        14313 + 14266 + 14606,
    ]

    # average
    data1 = [i / 3 for i in data1]
    data2 = [i / 3 for i in data2]
    data3 = [i / 3 for i in data3]

    print(data1)
    print(data2)
    print(data3)
    draw(data1, data2, data3)
