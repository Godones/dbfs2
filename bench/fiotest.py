import matplotlib.pyplot as plt


def fio2(name: str, data1: list, data2: list, data3: list):
    x_label = ["seq_write", "seq_read", "rand_write", "rand_read"]
    fs = ["ext3", "ext4", "dbfs"]

    plt.figure(figsize=(8, 6))

    width = 0.2

    x = [i for i in range(len(x_label))]
    x1 = x
    x2 = [i + width for i in x]
    x3 = [i + width * 2 for i in x]
    plt.bar(x1, data1, width=width, label="ext3", color=rgb2hex(233, 196, 107))
    plt.bar(x2, data2, width=width, label="ext4", color=rgb2hex(230, 111, 81))
    plt.bar(x3, data3, width=width, label="dbfs", color=rgb2hex(38, 70, 83))
    plt.title(name)
    plt.xticks(x2, x_label)
    plt.ylabel("NormalizedTput")
    plt.legend()
    path = format("./result/fiotest/%s.svg" % name)
    plt.savefig(path)
    plt.show()


def rgb2hex(r: int, g: int, b: int) -> str:
    return '#%02x%02x%02x' % (r, g, b)


def fio_barh():
    data = [350, 70, 227, 72]
    label = ["write", "rewrite", "randwrite", "rerandrw"]
    width = 0.2
    x = [i for i in range(len(label))]
    x1 = x
    x2 = [i + width for i in x]

    # plt.ylim(label)
    plt.barh(label, data, color=[rgb2hex(230, 111, 81), rgb2hex(230, 111, 81), rgb2hex(38, 70, 83), rgb2hex(38, 70, 83)])
    plt.title("rewrite-test")
    # plt.show()
    plt.savefig("./result/fiotest/fio_barh.svg")
    plt.xlabel("MB/s")
    plt.show()



def fio_small_test():

    data1 = [459,1779,1980]
    data2 = [366,800,778]
    data3 = [477,4399,4600]

    for (i, j) in enumerate(data1):
        data1[i] = j / data2[i]
    for (i, j) in enumerate(data3):
        data3[i] = j / data2[i]
    data2 = [1.0, 1.0, 1.0, ]

    x_label = ["seq_write", "seq_read", "rand_read"]

    plt.figure(figsize=(8, 6))

    # 柱状图宽度
    width = 0.2
    # 柱状图间距
    x = [i for i in range(len(x_label))]
    x1 = x
    x2 = [i + width for i in x]
    x3 = [i + width * 2 for i in x]
    plt.bar(x1, data1, width=width, label="ext3", color=rgb2hex(233, 196, 107))
    plt.bar(x2, data2, width=width, label="ext4", color=rgb2hex(230, 111, 81))
    plt.bar(x3, data3, width=width, label="dbfs", color=rgb2hex(38, 70, 83))
    plt.title("small file test")
    plt.xticks(x2, x_label)
    plt.ylabel("NormalizedTput")
    plt.legend()
    path = format("./result/fiotest/%s.svg" % "small file test")
    plt.savefig(path)
    plt.show()


if __name__ == '__main__':

    # ext3
    data1 = [385, 301.5, 376, 195]
    # ext4
    data2 = [116, 268.5, 70, 155]
    # dbfs
    data3 = [350, 41, 227, 40]

    # 以ext4为基准
    for (i, j) in enumerate(data1):
        data1[i] = j / data2[i]
    for (i, j) in enumerate(data3):
        data3[i] = j / data2[i]

    data2 = [1.0, 1.0, 1.0, 1.0]
    fio2("fio-test-1job", data1, data2, data3)

    data1 = [397, 370, 393, 280]
    data2 = [135, 351, 78, 230]
    data3 = [360, 41, 241, 39]
    for (i, j) in enumerate(data1):
        data1[i] = j / data2[i]
    for (i, j) in enumerate(data3):
        data3[i] = j / data2[i]
    data2 = [1.0, 1.0, 1.0, 1.0]
    fio2("fio-test-4job", data1, data2, data3)
    # ext3
    data1 = [385, 301.5, 376, 195]
    # ext4
    data2 = [116, 268.5, 70, 155]
    # dbfs
    data3 = [350, 41, 227, 40]

    data11 = [397, 370, 393, 280]
    data22 = [135, 351, 78, 230]
    data33 = [360, 41, 241, 39]

    for (i, j) in enumerate(data11):
        data11[i] = j / data1[i]
    for (i, j) in enumerate(data22):
        data22[i] = j / data2[i]
    for (i, j) in enumerate(data33):
        data33[i] = j / data3[i]

    fio2("fio-test-1job-4job", data11, data22, data33)

    fio_barh()
    fio_small_test()