IMG_PATH := .
EXT3_IMG := $(IMG_PATH)/ext3.img
EXT4_IMG := $(IMG_PATH)/ext4.img
BS := 1M
BS_COUNT := 20480

IMG_LIST := $(EXT3_IMG)  $(EXT4_IMG)
IMG_TOOL := mkfs.ext3  mkfs.ext4

EXT3_MOUNT_POINT:=$(IMG_PATH)/ext3
EXT4_MOUNT_POINT:=$(IMG_PATH)/ext4
DBFS_MOUNT_POINT := $(IMG_PATH)/dbfs

PJDFSTEST_PATH := ?

MOUNT_POINT := $(EXT3_MOUNT_POINT) $(EXT4_MOUNT_POINT) $(DBFS_MOUNT_POINT)

# every img has a tool
IMG_TOOL_INDEX := $(shell seq $(words $(IMG_LIST)))

DEPENDENCIES := fuse2fs python3


MDTEST_RESULT:= ./result/mdtest
EXT3_MDTEST_RESULT := $(MDTEST_RESULT)/ext3.txt
EXT4_MDTEST_RESULT := $(MDTEST_RESULT)/ext4.txt
DBFS_MDTEST_RESULT := $(MDTEST_RESULT)/dbfs.txt

FIO_RESULT:= ./result/fiotest
EXT3_FIO_RESULT := $(FIO_RESULT)/ext3.txt
EXT4_FIO_RESULT := $(FIO_RESULT)/ext4.txt
DBFS_FIO_RESULT := $(FIO_RESULT)/dbfs.txt



EXT_MOUNT_OPTIONS := -o allow_other,auto_unmount,default_permissions,rw,no_default_opts,async_read

FIO_LIST = clat lat slat bw iops

# Print variable for example
define mkfs
	if [ ! -f $1 ]; then \
		echo "create $1 using $2"; \
		sudo dd if=/dev/zero of=$1 bs=$(BS) count=$(BS_COUNT); \
		sudo $2 $1 ; \
	fi
endef

define fiopic
	$(foreach i, $(FIO_LIST), mv $(1)_$(i).1.log ./result/fiotest/$(1)_$(i).log;)
endef


define fiotest
	sudo sh -c 'echo 1 > /proc/sys/vm/drop_caches'
	sudo sleep 5
	sudo fio --name=test-file --directory=$(1) --rw=$(2) --bs=1m --size=$(3) --numjobs=$(4) --direct=1 --group_reporting > ./result/fiotest/$(2)_$(5).txt;
endef

define dependence_check
	if which $(1); then \
		echo "$(1) has been installed" ; \
	else \
		echo "install $(1)" ; \
		sudo apt-get install $(1) -y; \
	fi;
endef


define mountfs
	if mountpoint -q $2; then \
		echo "remount...";\
		sudo umount $2;\
	fi
	sudo e2fsck $1 -fy -y;
	sudo fuse2fs $(EXT_MOUNT_OPTIONS) $1 $2
endef


# install tools
pre_install:
	@$(foreach tool, $(DEPENDENCIES), $(call dependence_check, $(tool)))

# create ext3/ext4image and mount point
pre_file:
	@# checkout all mount point has been created
	@$(foreach mount, $(MOUNT_POINT), (if [ ! -d $(mount) ]; then \
		echo "create $(mount)"; \
		sudo mkdir $(mount); \
		sudo chmod 777 $(mount); \
	fi;);)
	@echo "Try mkfs"
	@# checkout the image has been created
	@$(foreach j, $(IMG_TOOL_INDEX), $(call mkfs,$(word $(j), $(IMG_LIST)), $(word $(j), $(IMG_TOOL)));)
	@#$(foreach fs, $(MOUNT_POINT),sudo rm $(fs)/* -rf;)


ext:
	@echo "Try mount fs"
	@$(foreach j, $(IMG_TOOL_INDEX), $(call mountfs,$(word $(j), $(IMG_LIST)), $(word $(j), $(MOUNT_POINT)));)
	@$(foreach fs, $(MOUNT_POINT),sudo rm $(fs)/* -rf;)


mdtest:
	@sudo mdtest -d $(EXT3_MOUNT_POINT) -b 6 -I 8 -z 3 > $(EXT3_MDTEST_RESULT)
	@echo "ext3 done"
	@sudo mdtest -d $(EXT4_MOUNT_POINT) -b 6 -I 8 -z 3 > $(EXT4_MDTEST_RESULT)
	@echo "ext4 done"
	@sudo mdtest -d $(DBFS_MOUNT_POINT) -b 6 -I 8 -z 3 > $(DBFS_MDTEST_RESULT)
	@echo "dbfs done"



fio_sw_1:
	@echo "RUN SEQ_WRITE"
	@$(call fiotest,./dbfs,write,15g,1,dbfs)
	@$(call fiotest,./ext3,write,15g,1,ext3)
	@$(call fiotest,./ext4,write,15g,1,ext4)


fio_sr_1:
	@echo "RUN SEQ_WRITE"
	@$(call fiotest,./dbfs,read,15g,1,dbfs)
	@$(call fiotest,./ext3,read,15g,1,ext3)
	@$(call fiotest,./ext4,read,15g,1,ext4)


fio_rw_1:
	@echo "RUN SEQ_RAND_WRITE"
	@$(call fiotest,./dbfs,randwrite,15g,1,dbfs)
	@$(call fiotest,./ext3,randwrite,15g,1,ext3)
	@$(call fiotest,./ext4,randwrite,15g,1,ext4)

fio_rr_1:
	@echo "RUN SEQ_RAND_WRITE"
	@$(call fiotest,./dbfs,randread,15g,1,dbfs)
	@$(call fiotest,./ext3,randread,15g,1,ext3)
	@$(call fiotest,./ext4,randread,15g,1,ext4)



fio_sw_4:
	@echo "RUN SEQ_WRITE"
	@$(call fiotest,./dbfs,write,3g,4,dbfs)
	@$(call fiotest,./ext3,write,3g,4,ext3)
	@$(call fiotest,./ext4,write,3g,4,ext4)


fio_sr_4:
	@echo "RUN SEQ_WRITE"
	@$(call fiotest,./dbfs,read,3g,4,dbfs)
	@$(call fiotest,./ext3,read,3g,4,ext3)
	@$(call fiotest,./ext4,read,3g,4,ext4)


fio_rw_4:
	@echo "RUN SEQ_RAND_WRITE"
	@$(call fiotest,./dbfs,randwrite,3g,4,dbfs)
	@$(call fiotest,./ext3,randwrite,3g,4,ext3)
	@$(call fiotest,./ext4,randwrite,3g,4,ext4)

fio_rr_4:
	@echo "RUN SEQ_RAND_WRITE"
	@$(call fiotest,./dbfs,randread,3g,4,dbfs)
	@$(call fiotest,./ext3,randread,3g,4,ext3)
	@$(call fiotest,./ext4,randread,3g,4,ext4)


#rfio:
#$(call fiopic,rw)


fbench:
	@echo "RUN FILEBENCH"
	@sudo sh -c 'echo 0 > /proc/sys/kernel/nmi_watchdog'
	@sudo filebench -f ./filebench/fileserver.f > ./result/filebench/fileserver.txt
	@sudo sh -c 'echo 1 > /proc/sys/vm/drop_caches'
	@sudo sleep 5
	@sudo filebench -f ./filebench/varmail.f > ./result/filebench/varmail.txt
	@sudo sh -c 'echo 1 > /proc/sys/vm/drop_caches'
	@sudo sleep 5
	@sudo filebench -f ./filebench/webserver.f > ./result/filebench/webserver.txt
	@sudo sh -c 'echo 1 > /proc/sys/vm/drop_caches'



clean:
	$(foreach fs, $(MOUNT_POINT),sudo rm $(fs)/* -rf;)

umount:
	@$(foreach fs, $(MOUNT_POINT),sudo umount $(fs);)

