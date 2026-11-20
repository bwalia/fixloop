import SwiftUI

extension Severity {
    var color: Color {
        switch self {
        case .critical: .red
        case .high: .orange
        case .medium: Color(red: 0.72, green: 0.47, blue: 0.0)
        case .low: .blue
        case .info: .gray
        }
    }

    var symbol: String {
        switch self {
        case .critical: "exclamationmark.octagon.fill"
        case .high: "exclamationmark.triangle.fill"
        case .medium: "exclamationmark.circle.fill"
        case .low: "info.circle.fill"
        case .info: "circle.fill"
        }
    }
}

struct SeverityBadge: View {
    let severity: Severity
    var count: Int?

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: severity.symbol)
            Text(verbatim: count.map { "\($0) \(severity.rawValue)" } ?? severity.label)
        }
        .font(.caption.weight(.semibold))
        .lineLimit(1)
        .fixedSize()
        .padding(.horizontal, 8)
        .padding(.vertical, 3)
        .foregroundStyle(severity.color)
        .background(severity.color.opacity(0.15), in: Capsule())
        .accessibilityElement(children: .combine)
    }
}

/// The Fixloop loop mark, drawn natively so it is crisp at any size.
struct LogoView: View {
    var body: some View {
        GeometryReader { geo in
            let s = min(geo.size.width, geo.size.height) / 32
            ZStack {
                Path { p in
                    p.addArc(center: CGPoint(x: 16 * s, y: 16 * s), radius: 10 * s,
                             startAngle: .degrees(0), endAngle: .degrees(-54.5), clockwise: false)
                }
                .stroke(.primary, style: StrokeStyle(lineWidth: 3 * s, lineCap: .round))
                Path { p in
                    p.move(to: CGPoint(x: 17.5 * s, y: 4.5 * s))
                    p.addLine(to: CGPoint(x: 22.5 * s, y: 7.9 * s))
                    p.addLine(to: CGPoint(x: 18.9 * s, y: 12.8 * s))
                }
                .stroke(.primary, style: StrokeStyle(lineWidth: 3 * s, lineCap: .round, lineJoin: .round))
                RoundedRectangle(cornerRadius: 1 * s)
                    .fill(Brand.accent)
                    .frame(width: 5 * s, height: 5 * s)
                    .position(x: 25 * s, y: 16.5 * s)
            }
        }
        .aspectRatio(1, contentMode: .fit)
        .accessibilityHidden(true)
    }
}
