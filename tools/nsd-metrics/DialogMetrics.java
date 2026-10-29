import java.awt.Font;
import java.awt.FontMetrics;
import java.awt.Graphics2D;
import java.awt.RenderingHints;
import java.awt.image.BufferedImage;

/// Writes the metrics of Java's logical font Dialog 12 the way Structorizer measures its
/// diagram texts (antialiased text, integer metrics), for `umlcraft-nsd`
/// (`crates/umlcraft-nsd/src/metrics/dialog12-<platform>.txt`):
///
///     platform <os> <java version>
///     plain|bold ascent <a> descent <d> height <h>
///     plain|bold <advance of U+0020> … <advance of U+00FF>
///     plain|bold extra U+2190=10 …
///
/// Advances are exact (bold ones are fractional on Linux); Java's `stringWidth` of a line is
/// the sum of its characters' advances, rounded. The font behind Dialog differs by
/// platform, so the tables do too. Run it with any JDK in source-file mode:
///
///     java -Djava.awt.headless=true tools/nsd-metrics/DialogMetrics.java > dialog12-linux.txt
public class DialogMetrics {
    /// Characters beyond Latin-1 that Structorizer diagrams use: arrows and comparison
    /// operators, the empty set of empty branches, typographic quotes and dashes, the euro.
    static final String EXTRA =
            "←→≠≤≥∅…€–—‘’“”";

    public static void main(String[] args) {
        Graphics2D g = new BufferedImage(1, 1, BufferedImage.TYPE_INT_RGB).createGraphics();
        g.setRenderingHint(RenderingHints.KEY_TEXT_ANTIALIASING,
                RenderingHints.VALUE_TEXT_ANTIALIAS_ON);
        g.setRenderingHint(RenderingHints.KEY_FRACTIONALMETRICS,
                RenderingHints.VALUE_FRACTIONALMETRICS_OFF);
        System.out.println("platform " + System.getProperty("os.name").replace(' ', '_') + " "
                + System.getProperty("java.version"));
        for (int style : new int[] {Font.PLAIN, Font.BOLD}) {
            String name = style == Font.BOLD ? "bold" : "plain";
            Font font = new Font("Dialog", style, 12);
            FontMetrics fm = g.getFontMetrics(font);
            System.out.println(name + " ascent " + fm.getAscent() + " descent " + fm.getDescent()
                    + " height " + fm.getHeight());
            StringBuilder sb = new StringBuilder(name);
            for (char c = 0x20; c <= 0xff; c++) {
                sb.append(' ').append(Character.isISOControl(c) ? "0" : advance(font, g, c));
            }
            System.out.println(sb);
            sb = new StringBuilder(name + " extra");
            for (char c : EXTRA.toCharArray()) {
                sb.append(String.format(" U+%04X=%s", (int) c, advance(font, g, c)));
            }
            System.out.println(sb);
        }
    }

    /// The exact advance, without trailing zeros (`7`, `7.375`).
    static String advance(Font font, Graphics2D g, char c) {
        double w = font.getStringBounds(String.valueOf(c), g.getFontRenderContext()).getWidth();
        return new java.math.BigDecimal(w).stripTrailingZeros().toPlainString();
    }
}
