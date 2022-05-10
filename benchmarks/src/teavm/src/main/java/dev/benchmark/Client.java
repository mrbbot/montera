package dev.benchmark;

import org.teavm.jso.JSBody;

public class Client {
    @JSBody(script = "window.fib = function(n) { return javaMethods.get('dev.benchmark.Fib.fib(I)I').invoke(n) };"
      + "window.gcd = function(a, b) { return javaMethods.get('dev.benchmark.Gcd.gcd(II)I').invoke(a, b) };"
      + "window.sum = function(a, b, c) { return javaMethods.get('dev.benchmark.Objects.tripleSum(III)I').invoke(a, b, c) };")
    public static native void onLoad();

    public static void main(String[] args) {
        onLoad();
    }
}
