import matplotlib.pyplot as plt


def filebench(ylabel: str, name: str, data1: list, data2: list, data3: list):
    x_label = ["mailserver","webserver", "fileserver"]
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
    plt.ylabel(ylabel)
    plt.legend()
    path = format("./result/filebench/%s.svg" % name)
    plt.savefig(path)
    plt.show()


def rgb2hex(r: int, g: int, b: int) -> str:
    return '#%02x%02x%02x' % (r, g, b)


if __name__ == '__main__':
    iop_mail = [504.671, 552.815, 836.593]
    iop_webserver = [11751, 10808, 2573]
    iop_fileserver = [2960, 2284, 784]

    ext3_iop = [iop_mail[0], iop_webserver[0], iop_fileserver[0]]
    ext4_iop = [iop_mail[1], iop_webserver[1], iop_fileserver[1]]
    dbfs_iop = [iop_mail[2], iop_webserver[2], iop_fileserver[2]]

    for (i, j) in enumerate(ext3_iop):
        ext3_iop[i] = j / ext4_iop[i]
    for (i, j) in enumerate(dbfs_iop):
        dbfs_iop[i] = j / ext4_iop[i]
    ext4_iop = [1.0, 1.0, 1.0]

    filebench("NormalizedIops", "filebench-iop", ext3_iop, ext4_iop, dbfs_iop)

    t_mail = [1.7, 1.9, 2.8]
    t_webserver = [62, 57, 14]
    t_fileserver = [70, 54, 18]

    ext3_throughput = [t_mail[0], t_webserver[0], t_fileserver[0]]
    ext4_throughput = [t_mail[1], t_webserver[1], t_fileserver[1]]
    dbfs_throughput = [t_mail[2], t_webserver[2], t_fileserver[2]]
    for (i, j) in enumerate(ext3_throughput):
        ext3_throughput[i] = j / ext4_throughput[i]
    for (i, j) in enumerate(dbfs_throughput):
        dbfs_throughput[i] = j / ext4_throughput[i]
    ext4_throughput = [1.0, 1.0, 1.0]
    filebench("NormalizedTput", "filebench-throughput", ext3_throughput, ext4_throughput, dbfs_throughput)
