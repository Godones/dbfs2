import matplotlib.pyplot as plt


def fio(name: str, job1_data: list, job2_data: list, job4_data: list):
    x_label = ["1-job", "2-job", "4-job"]
    # [ext2,ext4,jfs,dbfs]
    ext2_data = [job1_data[0], job2_data[0], job4_data[0]]
    ext4_data = [job1_data[1], job2_data[1], job4_data[1]]
    jfs_data = [job1_data[2], job2_data[2], job4_data[2]]
    dbfs_data = [job1_data[3], job2_data[3], job4_data[3]]

    plt.figure(figsize=(8, 6))

    # 柱状图宽度
    width = 0.2
    # 柱状图间距
    x = [i for i in range(len(x_label))]
    x1 = x
    x2 = [i + width for i in x]
    x3 = [i + width * 2 for i in x]
    x4 = [i + width * 3 for i in x]
    plt.bar(x1, ext2_data, width=width, label="ext2")
    plt.bar(x2, ext4_data, width=width, label="ext4")
    plt.bar(x3, jfs_data, width=width, label="jfs")
    plt.bar(x4, dbfs_data, width=width, label="dbfs")
    plt.title(name)
    plt.xticks(x1, x_label)
    plt.xlabel("job")
    plt.ylabel("MB/s")
    plt.legend()
    path = format("./result/fiotest/%s.svg" % name)
    plt.savefig(path)
    # plt.show()


if __name__ == '__main__':
    job1_data = [465 + 458 + 530, 326 + 315 + 303, 735 + 1618 + 1111, 13 + 15 + 16]
    job2_data = [871 + 897 + 866, 332 + 342 + 359, 490 + 749 + 1476, 24 + 28 + 32]
    job4_data = [1053 + 1030 + 996, 348 + 350 + 342, 1623 + 833 + 723, 64 + 66 + 62]
    # avg
    job1_data = [int(i / 3) for i in job1_data]
    job2_data = [int(i / 3) for i in job2_data]
    job4_data = [int(i / 3) for i in job4_data]
    fio("fio_seq_write", job1_data, job2_data, job4_data)

    job1_data = [298 + 507 + 473, 295 + 365 + 389, 573 + 515 + 613, 16 + 25 + 28]
    job2_data = [536 + 539 + 557, 538 + 514 + 633, 2456 + 1583 + 2541, 43 + 49 + 47]
    job4_data = [2674 + 2599 + 2197, 825 + 923 + 855, 3048 + 3282 + 2860, 1969 + 1932 + 1897]
    # avg
    job1_data = [int(i / 3) for i in job1_data]
    job2_data = [int(i / 3) for i in job2_data]
    job4_data = [int(i / 3) for i in job4_data]
    fio("fio_seq_read", job1_data, job2_data, job4_data)
    print(job1_data)
    print(job2_data)
    print(job4_data)

    job1_data = [595 + 586 + 608, 306 + 312 + 337, 913 + 245 + 652, 13 + 13 + 14]
    job2_data = [828 + 903 + 817, 334 + 340 + 366, 865 + 1603 + 373, 20 + 19 + 19]
    job4_data = [980 + 918 + 894, 344 + 355 + 354, 619 + 113 + 71, 40 + 40 + 33]
    fio("fio_rand_write", job1_data, job2_data, job4_data)

    job1_data = [356.0 + 254 + 250, 327.0 + 186 + 212, 107.0 + 238 + 238, 20.0 + 16 + 16]
    job2_data = [292 + 417 + 406, 210 + 359 + 315, 252 + 689 + 919, 56 + 93 + 86]
    job4_data = [2291 + 2606 + 2660, 854 + 774 + 880, 599 + 913 + 925, 1857 + 1882 + 1882]
    # avg
    job1_data = [int(i / 3) for i in job1_data]
    job2_data = [int(i / 3) for i in job2_data]
    job4_data = [int(i / 3) for i in job4_data]
    fio("fio_rand_read", job1_data, job2_data, job4_data)
