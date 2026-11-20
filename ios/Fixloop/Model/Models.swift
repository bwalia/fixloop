import Foundation

enum Severity: String, CaseIterable, Comparable, Sendable, Identifiable {
    case critical, high, medium, low, info

    var id: String { rawValue }

    /// Lower rank sorts first.
    var rank: Int {
        switch self {
        case .critical: 0
        case .high: 1
        case .medium: 2
        case .low: 3
        case .info: 4
        }
    }

    var label: String { rawValue.capitalized }

    static func < (a: Severity, b: Severity) -> Bool { a.rank < b.rank }
}

/// One finding, parsed from a Fixloop inline review comment.
struct Finding: Identifiable, Hashable, Sendable {
    let id: Int
    let severity: Severity
    let category: String
    let title: String
    let rationale: String
    let fix: String?
    let suggestion: String?
    let confidence: Int?
    let path: String
    let line: Int?
    let url: URL?
}

/// The summary review Fixloop posts on a pull request.
struct ReviewSummary: Hashable, Sendable {
    let headline: String
    let counts: [Severity: Int]
    let model: String?
    let submittedAt: Date?

    var total: Int { counts.values.reduce(0, +) }

    var worst: Severity? { counts.filter { $0.value > 0 }.keys.min() }
}

/// A pull request with its latest Fixloop review.
struct ReviewedPR: Identifiable, Hashable, Sendable {
    let repo: String // owner/name
    let number: Int
    let title: String
    let author: String
    let url: URL
    let updatedAt: Date
    let isOpen: Bool
    let summary: ReviewSummary

    var id: String { "\(repo)#\(number)" }
}
