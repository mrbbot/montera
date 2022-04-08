JAVA_HOME=/usr/local/Cellar/openjdk@11/11.0.12

SRC_CHEERPJ=src/cheerpj
cheerpj_clean:
	rm -rf $(SRC_CHEERPJ)/out
cheerpj_build:
	javac -d $(SRC_CHEERPJ)/out/production/cheerpj $(SRC_CHEERPJ)/src/*.java
	mkdir -p $(SRC_CHEERPJ)/out/artifacts/cheerpj_jar
	cd $(SRC_CHEERPJ)/out/production/cheerpj; jar cfm ../../artifacts/cheerpj_jar/cheerpj.jar ../../../src/META-INF/MANIFEST.MF *.class
	sdks/cheerpj-2.2/cheerpjfy.py $(SRC_CHEERPJ)/out/artifacts/cheerpj_jar/cheerpj.jar

SRC_GWT=src/gwt
gwt_clean:
	rm -rf $(SRC_GWT)/gwt-unitCache
	cd $(SRC_GWT); ant clean
gwt_build:
	cd $(SRC_GWT); ant build

SRC_HANDWRITTEN=src/handwritten
handwritten_clean:
	rm -rf $(SRC_HANDWRITTEN)/out-wasm
handwritten_build:
	mkdir -p $(SRC_HANDWRITTEN)/out-wasm
	wat2wasm $(SRC_HANDWRITTEN)/mod.wat -o $(SRC_HANDWRITTEN)/out-wasm/mod.wasm

SRC_JVM=src/jvm
jvm_clean:
	rm -rf $(SRC_JVM)/out-class
jvm_build:
	javac -d $(SRC_JVM)/out-class $(SRC_JVM)/*.java

SRC_JWEBASSEMBLY=src/jwebassembly
jwebassembly_clean:
	cd $(SRC_JWEBASSEMBLY); ./gradlew clean
jwebassembly_build:
	cd $(SRC_JWEBASSEMBLY); ./gradlew wasm

SRC_MONTERA=src/montera
montera_clean:
	rm -rf $(SRC_MONTERA)/{out-class,out-wasm}
montera_build:
	javac -d $(SRC_MONTERA)/out-class $(SRC_MONTERA)/*.java
	RUST_LOG=info montera --optimise --output $(SRC_MONTERA)/out-wasm/mod.wasm $(SRC_MONTERA)/out-class/*.class

SRC_TEAVM=src/teavm
teavm_clean:
	cd $(SRC_TEAVM); JAVA_HOME=$(JAVA_HOME) mvn clean
teavm_build:
	cd $(SRC_TEAVM); JAVA_HOME=$(JAVA_HOME) mvn package