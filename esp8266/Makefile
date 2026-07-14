#
# This is a project Makefile. It is assumed the directory this Makefile resides in is a
# project subdirectory.
#

PROJECT_NAME := esp8266

include $(IDF_PATH)/make/project.mk

CFLAGS += -Wno-error=maybe-uninitialized
CXXFLAGS += -Wno-error=maybe-uninitialized
.clangd :
	$(file >$@, CompileFlags:)
	$(file >>$@,  Add: [$(COMPONENT_INCLUDES:%=-I%,) -D__ESP_FILE__="main.c"])
