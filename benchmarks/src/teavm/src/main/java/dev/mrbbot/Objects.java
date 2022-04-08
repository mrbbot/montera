package dev.mrbbot;

public class Objects {
    public static class Pair {
        private final int a;
        private final int b;

        public Pair(int a, int b) {
            this.a = a;
            this.b = b;
        }

        public int sum() {
            return this.a + this.b;
        }
    }

    public static class Triple extends Pair {
        private final int c;

        public Triple(int a, int b, int c) {
            super(a, b);
            this.c = c;
        }

        @Override
        public int sum() {
            return super.sum() + c;
        }
    }

    public static int tripleSum(int a, int b, int c) {
        Triple triple = new Triple(a, b, c);
        return triple.sum();
    }
}