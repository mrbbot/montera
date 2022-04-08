public class Main {
    // Run benchmarks
    public static void main(String[] args) {
        // Calculate the first 40 fibonacci numbers
        long fibStart = System.nanoTime();
        for (int i = 1; i <= 40; i++) Fib.fib(i);
        long fibTime = System.nanoTime() - fibStart;

        // Calculate the gcd's of all combinations of naturals up to 2^11
        int max = (int) Math.pow(2, 11);
        long gcdStart = System.nanoTime();
        for (int a = 1; a < max; a++) {
            for (int b = 1; b < max; b++) Gcd.gcd(a, b);
        }
        long gcdTime = System.nanoTime() - gcdStart;

        // Calculate the sum of all {i, i+1, i+2}s for natural i's up to 4000
        long sumStart = System.nanoTime();
        for (int i = 1; i < 4000; i++) Objects.tripleSum(i, i + 1, i + 2);
        long sumTime = System.nanoTime() - sumStart;

        // Convert nanoseconds to milliseconds
        double fibTimeMs = ((double) fibTime) / 1000000.0;
        double gcdTimeMs = ((double) gcdTime) / 1000000.0;
        double sumTimeMs = ((double) sumTime) / 1000000.0;
        System.out.printf("{\"fibTime\": %f, \"gcdTime\": %f, \"sumTime\": %f}\n", fibTimeMs, gcdTimeMs, sumTimeMs);
    }
}