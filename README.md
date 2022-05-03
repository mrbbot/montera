# ⚙️ `montera`

Final year university project: a *highly* experimental JVM bytecode to WebAssembly compiler

> ⚠️ Do **NOT** use this for serious projects yet! It's currently
> missing support for key features like the Java standard
> library, garbage collection, strings, arrays and exceptions.
> For more complete alternatives, see:
> - JWebAssembly (https://github.com/i-net-software/JWebAssembly)   
> - CheerpJ (https://leaningtech.com/cheerpj/)           
> - TeaVM (https://teavm.org)    
> - Google Web Toolkit (http://www.gwtproject.org/)

## Building

To generate a release build:

```shell
$ cargo build --release
```

## Testing

To run unit and integration tests, make sure `javac` and `dot` executables are in the system `PATH`, then run:

```shell
$ cargo test
```

## Benchmarking

To run benchmarks, make sure:

- [Node.js 16 LTS](https://nodejs.org/en/) is installed
- [Python 3](https://www.python.org/) is installed
- [OpenJDK 7 to 11](https://openjdk.java.net/) is installed, with the `javac` executable in the system `PATH`, and `JAVA_HOME` pointing to the installation
- [CheerpJ 2.2](https://leaningtech.com/download-cheerpj/) is installed in `benchmarks/sdks/cheerpj-2.2`
- [GWT 2.9.0](https://www.gwtproject.org/download.html) is installed in `benchmarks/sdks/gwt-2.9.0`

...then run:

```shell
$ cargo build --release  # Build project for benchmarking
$ export JAVA_HOME=/usr/local/Cellar/openjdk@11/11.0.12  # e.g. Java 11 installed using `brew` on macOS
$ cd benchmarks

$ npm install                      # Install Node.js dependencies
$ npm run bench:build              # Run compilation time benchmark
$ npm run bench:runtime            # Run runtime performance benchmark
$ npm run bench:size               # Run download size benchmark

$ pip install -r requirements.txt  # Install Python dependencies (may want to create virtualenv)
$ npm run plot:build               # Plot compilation time benchmark results
$ npm run plot:runtime             # Plot runtime performance benchmark results
$ npm run plot:size                # Plot download size benchmark results
```

## Acknowledgements

Bytecode structuring algorithms in [`src/function/structure`](./src/function/structure) are based on those described by
Cristina Cifuentes in their [**Reverse Compilation Techniques** PhD thesis](https://citeseerx.ist.psu.edu/viewdoc/download?doi=10.1.1.105.6048&rep=rep1&type=pdf).

The immediate dominance algorithm in [`src/graph/dominators.rs`](./src/graph/dominators.rs) is based on that described by Keith D. Cooper, Timothy J. Harvey, and Ken Kennedy
in [**A Simple, Fast Dominance Algorithm**](https://www.cs.rice.edu/~keith/EMBED/dom.pdf).