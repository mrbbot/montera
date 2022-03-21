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

## Acknowledgements

Bytecode structuring algorithms in [`src/function/structure`](./src/function/structure) are based on those described by
Cristina Cifuentes in their [**Reverse Compilation Techniques** PhD thesis](https://citeseerx.ist.psu.edu/viewdoc/download?doi=10.1.1.105.6048&rep=rep1&type=pdf).

The immediate dominance algorithm in [`src/graph/dominators.rs`](./src/graph/dominators.rs) is based on that described by Keith D. Cooper, Timothy J. Harvey, and Ken Kennedy
in [**A Simple, Fast Dominance Algorithm**](https://www.cs.rice.edu/~keith/EMBED/dom.pdf).