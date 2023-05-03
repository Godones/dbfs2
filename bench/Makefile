IMG_PATH := .
EXT2_IMG := $(IMG_PATH)/ext2.img
EXT4_IMG := $(IMG_PATH)/ext4.img
JFS_IMG := $(IMG_PATH)/jfs.img
BS := 4M
BS_COUNT := 1024

IMG_LIST := $(EXT2_IMG)  $(EXT4_IMG)
IMG_TOOL := mkfs.ext2  mkfs.ext4

EXT2_MOUNT_POINT:=$(IMG_PATH)/ext2
EXT4_MOUNT_POINT:=$(IMG_PATH)/ext4
JFS_MOUNT_POINT := $(IMG_PATH)/jfs
DBFS_MOUNT_POINT := $(IMG_PATH)/dbfs

PJDFSTEST_PATH := ?

MOUNT_POINT := $(EXT2_MOUNT_POINT) $(EXT4_MOUNT_POINT) $(JFS_MOUNT_POINT) $(DBFS_MOUNT_POINT)

# every img has a tool
IMG_TOOL_INDEX := $(shell seq $(words $(IMG_LIST)))

DEPENDENCIES := fuse2fs python3 sqlite3

JFS_DB := $(IMG_PATH)/myjfs.db
JFS_NAME := myjfs

MDTEST_RESULT:= ./result/mdtest
EXT2_MDTEST_RESULT := $(MDTEST_RESULT)/ext2.txt
EXT4_MDTEST_RESULT := $(MDTEST_RESULT)/ext4.txt
DBFS_MDTEST_RESULT := $(MDTEST_RESULT)/dbfs.txt
JFS_MDTEST_RESULT := $(MDTEST_RESULT)/jfs.txt

FIO_RESULT:= ./result/fiotest
EXT2_FIO_RESULT := $(FIO_RESULT)/ext2.txt
EXT4_FIO_RESULT := $(FIO_RESULT)/ext4.txt
DBFS_FIO_RESULT := $(FIO_RESULT)/dbfs.txt
JFS_FIO_RESULT := $(FIO_RESULT)/jfs.txt



EXT_MOUNT_OPTIONS := -o allow_other,auto_unmount,default_permissions,rw,no_default_opts,async_read
JFS_MOUNT_OPTIONS := -o allow_other,default_permissions,rw,async,writeback_cache

# Print variable for example
define mkfs
	if [ $1 = $(JFS_IMG) ]; then \
		if [ ! -f $(JFS_DB) ]; then \
			juicefs format sqlite3://$(JFS_DB) $(JFS_NAME); \
		fi; \
	else \
		if [ ! -f $1 ]; then \
			echo "create $1 using $2"; \
		fi; \
		sudo dd if=/dev/zero of=$1 bs=$(BS) count=$(BS_COUNT); \
		sudo $2 $1 ; \
	fi
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
	if [ $1 = $(JFS_IMG) ]; then \
		if mountpoint -q $2; then \
			sudo juicefs umount $2;\
		fi; \
		sudo juicefs mount $(JFS_MOUNT_OPTIONS) sqlite3://$(JFS_DB) $2 -d; \
		sudo rm -rf $2/*; \
	else \
		if mountpoint -q $2; then \
			sudo umount $2;\
		fi;\
		sudo e2fsck $1 -fy; \
		sudo fuse2fs $(EXT_MOUNT_OPTIONS) $1 $2; \
		sudo rm -rf $2/*; \
	fi
endef


# install fuse2fs tool
pre_install:
	@# checkout whether fuse2fs has been installed
	@$(foreach tool, $(DEPENDENCIES), $(call dependence_check, $(tool)))
	@# checkout whether juicefs has been installed
	@if which juicefs; then \
		echo "juicefs has been installed" ; \
	else \
		echo "install juicefs" ; \
		curl -sSL https://d.juicefs.com/install | sh - ; \
	fi

# create ext2/ext3/ext4/jfs image and mount point
pre_file:
	@# checkout all mount point has been created
	@$(foreach mount, $(MOUNT_POINT), (if [ ! -d $(mount) ]; then \
		echo "create $(mount)"; \
		sudo mkdir $(mount); \
		sudo chmod 777 $(mount); \
	fi;);)
	@echo "Try mkfs"
	@# checkout the image has been created
	@$(call mkfs,$(JFS_IMG), $(JFS_IMG))
	@$(foreach j, $(IMG_TOOL_INDEX), $(call mkfs,$(word $(j), $(IMG_LIST)), $(word $(j), $(IMG_TOOL)));)
	@# mount all image
	@echo "Try mount fs"
	@$(call mountfs,$(JFS_IMG), $(JFS_MOUNT_POINT))
	@$(foreach j, $(IMG_TOOL_INDEX), $(call mountfs,$(word $(j), $(IMG_LIST)), $(word $(j), $(MOUNT_POINT)));)

dbfs:
	@cd .. && cargo run --release --example fuse -- --auto_unmount $(DBFS_MOUNT_POINT)


mdtest:
	@sudo mdtest -d $(EXT2_MOUNT_POINT) -b 6 -I 8 -z 2 > $(EXT2_MDTEST_RESULT)
	@echo "ext2 done"
	@sudo mdtest -d $(EXT4_MOUNT_POINT) -b 6 -I 8 -z 2 > $(EXT4_MDTEST_RESULT)
	@echo "ext4 done"
	@sudo mdtest -d $(JFS_MOUNT_POINT) -b 6 -I 8 -z 2 > $(JFS_MDTEST_RESULT)
	@echo "jfs done"
	@sudo mdtest -d $(DBFS_MOUNT_POINT) -b 6 -I 8 -z 2 > $(DBFS_MDTEST_RESULT)
	@echo "dbfs done"



fio_seq_write:
	@echo "RUN SEQ_WRITE"
	@$(foreach fs, $(MOUNT_POINT),sudo fio --name=$(fs) --directory=$(fs)  --rw=write --bs=1m --size=1g --numjobs=1 --direct=1 --group_reporting;)
	@echo "RUN SEQ_WRITE 2 jobs"
	@$(foreach fs, $(MOUNT_POINT),sudo fio --name=$(fs) --directory=$(fs)  --rw=write --bs=1m --size=512m --numjobs=2 --direct=1 --group_reporting;)
	@echo "RUN SEQ_WRITE 4 jobs"
	@$(foreach fs, $(MOUNT_POINT),sudo fio --name=$(fs) --directory=$(fs)  --rw=write --bs=1m --size=256m --numjobs=4 --direct=1 --group_reporting;)

fio_seq_read:
	@echo "RUN SEQ_WRITE"
	@$(foreach fs, $(MOUNT_POINT),sudo fio --name=$(fs) --directory=$(fs)  --rw=read --bs=1m --size=1g --numjobs=1 --direct=1 --group_reporting;)
	@echo "RUN SEQ_WRITE 2 jobs"
	@$(foreach fs, $(MOUNT_POINT),sudo fio --name=$(fs) --directory=$(fs)  --rw=read --bs=1m --size=512m --numjobs=2 --direct=1 --group_reporting;)
	@echo "RUN SEQ_WRITE 4 jobs"
	@$(foreach fs, $(MOUNT_POINT),sudo fio --name=$(fs) --directory=$(fs)  --rw=read --bs=1m --size=256m --numjobs=4 --direct=1 --group_reporting;)


fio_rand_write:
	@echo "RUN SEQ_RAND_WRITE"
	@$(foreach fs, $(MOUNT_POINT),sudo fio --name=$(fs) --directory=$(fs)  --rw=randwrite --bs=1m --size=1g --numjobs=1 --direct=1 --group_reporting;)
	@echo "RUN SEQ_RAND_WRITE 2 jobs"
	@$(foreach fs, $(MOUNT_POINT),sudo fio --name=$(fs) --directory=$(fs)  --rw=randwrite --bs=1m --size=512m --numjobs=2 --direct=1 --group_reporting;)
	@echo "RUN SEQ_RAND_WRITE 4 jobs"
	@$(foreach fs, $(MOUNT_POINT),sudo fio --name=$(fs) --directory=$(fs)  --rw=randwrite --bs=1m --size=256m --numjobs=4 --direct=1 --group_reporting;)


fio_rand_read:
	@echo "RUN SEQ_RAND_WRITE"
	@$(foreach fs, $(MOUNT_POINT),sudo fio --name=$(fs) --directory=$(fs)  --rw=randread --bs=1m --size=1g --numjobs=1 --direct=1 --group_reporting;)
	@echo "RUN SEQ_RAND_WRITE 2 jobs"
	@$(foreach fs, $(MOUNT_POINT),sudo fio --name=$(fs) --directory=$(fs)  --rw=randread --bs=1m --size=512m --numjobs=2 --direct=1 --group_reporting;)
	@echo "RUN SEQ_RAND_WRITE 4 jobs"
	@$(foreach fs, $(MOUNT_POINT),sudo fio --name=$(fs) --directory=$(fs)  --rw=randread --bs=1m --size=256m --numjobs=4 --direct=1 --group_reporting;)



clean:
	$(foreach fs, $(MOUNT_POINT),sudo rm $(fs)/* -rf;)


mdtest-sig:
	@sudo mdtest -d $(DBFS_MOUNT_POINT) -b 6 -I 8 -z 2
	@echo "dbfs done"


fio_seq_write_sig:
	@sudo fio --name=dbfs --directory=$(DBFS_MOUNT_POINT)  --rw=write --bs=1m --size=1g --numjobs=1 --direct=1 --group_reporting
	@sudo fio --name=dbfs --directory=$(DBFS_MOUNT_POINT)  --rw=write --bs=1m --size=512m --numjobs=2 --direct=1 --group_reporting
	@sudo fio --name=dbfs --directory=$(DBFS_MOUNT_POINT)  --rw=write --bs=1m --size=256m --numjobs=4 --direct=1 --group_reporting


fio_seq_read_sig:
	@sudo fio --name=dbfs --directory=$(DBFS_MOUNT_POINT)  --rw=read --bs=1m --size=1g --numjobs=1 --direct=1 --group_reporting
	@sudo fio --name=dbfs --directory=$(DBFS_MOUNT_POINT)  --rw=read --bs=1m --size=512m --numjobs=2 --direct=1 --group_reporting
	@sudo fio --name=dbfs --directory=$(DBFS_MOUNT_POINT)  --rw=read --bs=1m --size=256m --numjobs=4 --direct=1 --group_reporting

fio_rand_write_sig:
	@sudo fio --name=dbfs --directory=$(DBFS_MOUNT_POINT)  --rw=randwrite --bs=1m --size=1g --numjobs=1 --direct=1 --group_reporting
	@sudo fio --name=dbfs --directory=$(DBFS_MOUNT_POINT)  --rw=randwrite --bs=1m --size=512m --numjobs=2 --direct=1 --group_reporting
	@sudo fio --name=dbfs --directory=$(DBFS_MOUNT_POINT)  --rw=randwrite --bs=1m --size=256m --numjobs=4 --direct=1 --group_reporting


fio_rand_read_sig:
	@sudo fio --name=dbfs --directory=$(DBFS_MOUNT_POINT)  --rw=randread --bs=1m --size=1g --numjobs=1 --direct=1 --group_reporting
	@sudo fio --name=dbfs --directory=$(DBFS_MOUNT_POINT)  --rw=randread --bs=1m --size=512m --numjobs=2 --direct=1 --group_reporting
	@sudo fio --name=dbfs --directory=$(DBFS_MOUNT_POINT)  --rw=randread --bs=1m --size=256m --numjobs=4 --direct=1 --group_reporting
