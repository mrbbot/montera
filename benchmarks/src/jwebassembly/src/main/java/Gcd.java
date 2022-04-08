import de.inetsoftware.jwebassembly.api.annotation.Export;

public class Gcd {
  @Export
  public static int gcd(int a, int b) {
    while (b != 0) {
      int tmp = b;
      b = a % b;
      a = tmp;
    }
    return a;
  }
}