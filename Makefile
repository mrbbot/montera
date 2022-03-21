# Compiles all Java files in DIR https://stackoverflow.com/a/53138757
JAVA_DIR := ./java
CLASS_DIR := ./java/target
SOURCES := $(shell find $(JAVA_DIR) -type f -name '*.java') # https://stackoverflow.com/a/2483203
CLASSES := $(patsubst $(JAVA_DIR)/%.java,$(CLASS_DIR)/%.class,$(SOURCES))

all: $(CLASSES)

$(CLASS_DIR)/%.class: $(JAVA_DIR)/%.java
	javac -d $(CLASS_DIR) $<
