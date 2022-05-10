package dev.benchmark.client;

import jsinterop.annotations.JsType;

@JsType
public class Gcd {
    public static int gcd(int a, int b) {
        while (b != 0) {
            int tmp = b;
            b = a % b;
            a = tmp;
        }
        return a;
    }
}