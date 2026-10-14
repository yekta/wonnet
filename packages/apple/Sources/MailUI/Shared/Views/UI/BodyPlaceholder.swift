import SwiftUI

/// Where a message's page will be while it is on its way or being drawn: a bar for every line
/// of its text and a block for every image, so the thread is about as tall as it will be and
/// barely moves when the page comes. `shape` is the page's lines, as the core gives them.
struct BodyPlaceholder: View {
    let shape: [ShapeLine]

    /// The page's line: its text is 15px on 1.55.
    private static let lineHeight: CGFloat = 23.25
    private static let barHeight: CGFloat = 10
    /// An average character of the page's text, with what wrapping leaves at the ends of lines.
    private static let characterWidth: CGFloat = 7.2
    /// Past this the placeholder is off the screen.
    private static let tallest: CGFloat = 1600
    /// How far full lines reach, as ragged as text.
    private static let reach: [CGFloat] = [1, 0.94, 0.97, 0.9, 0.98]

    var body: some View {
        LinesLayout(shape: shape) {
            Canvas { context, size in
                for block in Self.drawing(shape, width: size.width).blocks {
                    context.fill(Path(roundedRect: block, cornerRadius: Radius.xs), with: .color(Theme.cardAccent.color))
                }
            }
        }
        .accessibilityHidden(true)
    }

    struct Drawing {
        var blocks: [CGRect] = []
        var height: CGFloat = 0
    }

    /// The bars and blocks at this width, and how tall they stand: at least a line.
    static func drawing(_ shape: [ShapeLine], width: CGFloat) -> Drawing {
        var drawing = Drawing()
        guard width > 0 else { return drawing }
        let perLine = max(1, Int(width / characterWidth))
        func bar(_ fraction: CGFloat) {
            drawing.blocks.append(CGRect(x: 0, y: drawing.height + (lineHeight - barHeight) / 2, width: width * fraction, height: barHeight))
            drawing.height += lineHeight
        }
        for line in shape {
            guard drawing.height < tallest else { break }
            switch line {
            case .text(let length) where length <= 0:
                drawing.height += lineHeight
            case .text(let length):
                let full = (length - 1) / perLine
                for _ in 0..<full { bar(reach[drawing.blocks.count % reach.count]) }
                bar(max(CGFloat(length - full * perLine) / CGFloat(perLine), 0.06))
            case .image(let imageWidth, let imageHeight):
                // As the page shows it: never wider than the column, its proportions kept.
                let shown = min(CGFloat(max(imageWidth, 1)), width)
                let height = CGFloat(imageHeight) * shown / CGFloat(max(imageWidth, 1))
                drawing.blocks.append(CGRect(x: 0, y: drawing.height, width: shown, height: height))
                drawing.height += height
            }
        }
        drawing.height = max(drawing.height, lineHeight)
        return drawing
    }

    /// As wide as it is offered and as tall as its lines at that width.
    private struct LinesLayout: Layout {
        let shape: [ShapeLine]

        func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
            let width = proposal.width ?? 0
            return CGSize(width: width, height: BodyPlaceholder.drawing(shape, width: width).height)
        }

        func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
            subviews.first?.place(at: bounds.origin, proposal: ProposedViewSize(bounds.size))
        }
    }
}
