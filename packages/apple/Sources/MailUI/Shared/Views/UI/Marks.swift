import SwiftUI

/// The app's icon drawn small: its W on a tile, dark on light and light on dark, as Icon
/// Composer makes it.
struct AppMark: View {
    var size: CGFloat = 64

    private static let w = "M17.47 14.216l2.71-10.727H24l-4.55 16.715h-3.843l-3.56-11.08-3.583 11.08H4.55L0 3.49h3.96l2.76 10.703L10.184 3.49h3.819l3.466 10.727z"

    var body: some View {
        let side = size * Platform.scale
        let radius = side * 0.225
        VectorPath(Self.w, box: 24)
            .fill(Tokens.emphasizedForeground.color)
            .frame(width: side * 0.46, height: side * 0.46)
            .frame(width: side, height: side)
            .background(RoundedRectangle(cornerRadius: radius, style: .continuous).fill(Tokens.card.color).shadow(.sm))
            .overlay(RoundedRectangle(cornerRadius: radius, style: .continuous).strokeBorder(Tokens.border.color, lineWidth: Theme.hairline))
            .accessibilityHidden(true)
    }
}

/// Google's G, in its four colours, as its sign-in buttons carry it.
struct GoogleMark: View {
    var size: CGFloat = 20

    private static let parts: [(String, ThemeColor)] = [
        ("M45.12 24.5c0-1.56-.14-3.06-.4-4.5H24v8.51h11.84c-.51 2.75-2.06 5.08-4.39 6.64v5.52h7.11c4.16-3.83 6.56-9.47 6.56-16.17z", Tokens.googleBlue),
        ("M24 46c5.94 0 10.92-1.97 14.56-5.33l-7.11-5.52c-1.97 1.32-4.49 2.1-7.45 2.1-5.73 0-10.58-3.87-12.31-9.07H4.34v5.7C7.96 41.07 15.4 46 24 46z", Tokens.googleGreen),
        ("M11.69 28.18C11.25 26.86 11 25.45 11 24s.25-2.86.69-4.18v-5.7H4.34C2.85 17.09 2 20.45 2 24c0 3.55.85 6.91 2.34 9.88l7.35-5.7z", Tokens.googleYellow),
        ("M24 10.75c3.23 0 6.13 1.11 8.41 3.29l6.31-6.31C34.91 4.18 29.93 2 24 2 15.4 2 7.96 6.93 4.34 13.12l7.35 5.7c1.73-5.2 6.58-9.07 12.31-9.07z", Tokens.googleRed),
    ]

    var body: some View {
        ZStack {
            ForEach(Array(Self.parts.enumerated()), id: \.offset) { _, part in
                VectorPath(part.0, box: 48).fill(part.1.color)
            }
        }
        .frame(width: size * Platform.scale, height: size * Platform.scale)
        .accessibilityHidden(true)
    }
}

/// Microsoft's four squares, as its sign-in buttons carry them.
struct MicrosoftMark: View {
    var size: CGFloat = 20

    private static let parts: [(String, ThemeColor)] = [
        ("M1 1H11V11H1Z", Tokens.microsoftRed),
        ("M12 1H22V11H12Z", Tokens.microsoftGreen),
        ("M1 12H11V22H1Z", Tokens.microsoftBlue),
        ("M12 12H22V22H12Z", Tokens.microsoftYellow),
    ]

    var body: some View {
        ZStack {
            ForEach(Array(Self.parts.enumerated()), id: \.offset) { _, part in
                VectorPath(part.0, box: 23).fill(part.1.color)
            }
        }
        .frame(width: size * Platform.scale, height: size * Platform.scale)
        .accessibilityHidden(true)
    }
}

/// A shape from an SVG path's `d`, drawn in a square `box` wide, scaled to the frame it is
/// given. Reads the commands the marks use: M, L, H, V, C, S and Z, absolute or relative.
struct VectorPath: Shape {
    let d: String
    let box: CGFloat

    init(_ d: String, box: CGFloat) {
        self.d = d
        self.box = box
    }

    func path(in rect: CGRect) -> Path {
        var path = Path()
        var at = CGPoint.zero
        var start = CGPoint.zero
        var control: CGPoint?
        var command: Character = "M"
        var numbers: [CGFloat] = []
        let tokens = Self.tokens(d)

        func point(_ x: CGFloat, _ y: CGFloat, relative: Bool) -> CGPoint {
            relative ? CGPoint(x: at.x + x, y: at.y + y) : CGPoint(x: x, y: y)
        }

        func run() {
            let relative = command.isLowercase
            switch command.uppercased() {
            case "M":
                guard numbers.count >= 2 else { return }
                at = point(numbers[0], numbers[1], relative: relative)
                start = at
                path.move(to: at)
                for pair in stride(from: 2, to: numbers.count - 1, by: 2) {
                    at = point(numbers[pair], numbers[pair + 1], relative: relative)
                    path.addLine(to: at)
                }
                control = nil
            case "L":
                for pair in stride(from: 0, to: numbers.count - 1, by: 2) {
                    at = point(numbers[pair], numbers[pair + 1], relative: relative)
                    path.addLine(to: at)
                }
                control = nil
            case "H":
                for x in numbers {
                    at = CGPoint(x: relative ? at.x + x : x, y: at.y)
                    path.addLine(to: at)
                }
                control = nil
            case "V":
                for y in numbers {
                    at = CGPoint(x: at.x, y: relative ? at.y + y : y)
                    path.addLine(to: at)
                }
                control = nil
            case "C":
                for six in stride(from: 0, to: numbers.count - 5, by: 6) {
                    let first = point(numbers[six], numbers[six + 1], relative: relative)
                    let second = point(numbers[six + 2], numbers[six + 3], relative: relative)
                    let end = point(numbers[six + 4], numbers[six + 5], relative: relative)
                    path.addCurve(to: end, control1: first, control2: second)
                    control = second
                    at = end
                }
            case "S":
                for four in stride(from: 0, to: numbers.count - 3, by: 4) {
                    let first = control.map { CGPoint(x: 2 * at.x - $0.x, y: 2 * at.y - $0.y) } ?? at
                    let second = point(numbers[four], numbers[four + 1], relative: relative)
                    let end = point(numbers[four + 2], numbers[four + 3], relative: relative)
                    path.addCurve(to: end, control1: first, control2: second)
                    control = second
                    at = end
                }
            case "Z":
                path.closeSubpath()
                at = start
                control = nil
            default:
                return
            }
        }

        for token in tokens {
            if let number = Double(token) {
                numbers.append(CGFloat(number))
                continue
            }
            run()
            command = token.first ?? "M"
            numbers = []
            guard command.uppercased() == "Z" else { continue }
            run()
            command = " "
        }
        run()

        let scale = min(rect.width, rect.height) / box
        let fit = CGAffineTransform(translationX: rect.minX, y: rect.minY).scaledBy(x: scale, y: scale)
        return path.applying(fit)
    }

    /// The commands and the numbers of `d`, which SVG packs together: "-.14-3.06.69".
    private static func tokens(_ d: String) -> [String] {
        var tokens: [String] = []
        var number = ""
        func flush() {
            guard !number.isEmpty else { return }
            tokens.append(number)
            number = ""
        }
        for character in d {
            if character.isLetter {
                flush()
                tokens.append(String(character))
            } else if character == "-" {
                flush()
                number = "-"
            } else if character == "." {
                if number.contains(".") { flush() }
                number.append(".")
            } else if character.isNumber {
                number.append(character)
            } else {
                flush()
            }
        }
        flush()
        return tokens
    }
}
