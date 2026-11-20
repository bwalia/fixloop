import Foundation

/// Parses the Markdown that Fixloop posts (see crates/fixloop-agents/src/reviewer/render.rs).
enum ReviewParser {
    static let summaryHeading = "### Fixloop review"

    static func isFixloopSummary(_ body: String) -> Bool {
        body.trimmingCharacters(in: .whitespacesAndNewlines).hasPrefix(summaryHeading)
    }

    static func isFixloopComment(_ body: String) -> Bool {
        body.contains("<sub>Fixloop · confidence")
    }

    // MARK: Summary

    static func parseSummary(_ body: String, submittedAt: Date? = nil) -> ReviewSummary? {
        guard isFixloopSummary(body) else { return nil }
        let lines = body.components(separatedBy: "\n").map { $0.trimmingCharacters(in: .whitespaces) }
        // Headline is the first non-empty line after the heading, e.g. "1 critical, 1 high."
        let headline = lines.dropFirst().first { !$0.isEmpty } ?? ""
        var counts: [Severity: Int] = [:]
        let lowered = headline.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "."))
        if lowered != "no problems found" {
            for part in lowered.components(separatedBy: ",") {
                let bits = part.trimmingCharacters(in: .whitespaces).split(separator: " ")
                if bits.count == 2, let n = Int(bits[0]), let s = Severity(rawValue: String(bits[1])) {
                    counts[s] = n
                }
            }
        }
        var model: String?
        if let m = lines.first(where: { $0.hasPrefix("<sub>Model: ") }) {
            let rest = m.dropFirst("<sub>Model: ".count)
            model = rest.components(separatedBy: " · ").first
        }
        return ReviewSummary(headline: headline, counts: counts, model: model, submittedAt: submittedAt)
    }

    // MARK: Inline comment

    static func parseComment(id: Int, body: String, path: String, line: Int?, url: URL?) -> Finding? {
        guard isFixloopComment(body) else { return nil }
        var text = body

        // Header: **🔴 critical · security: Title**
        guard text.hasPrefix("**"), let close = text.range(of: "**", range: text.index(text.startIndex, offsetBy: 2)..<text.endIndex) else {
            return nil
        }
        let header = String(text[text.index(text.startIndex, offsetBy: 2)..<close.lowerBound])
        text = String(text[close.upperBound...])

        let headerParts = header.components(separatedBy: " · ")
        guard headerParts.count >= 2 else { return nil }
        let badgeWords = headerParts[0].split(separator: " ")
        guard let sevWord = badgeWords.last, let severity = Severity(rawValue: String(sevWord)) else { return nil }
        let rest = headerParts.dropFirst().joined(separator: " · ")
        let category: String
        let title: String
        if let colon = rest.range(of: ": ") {
            category = String(rest[..<colon.lowerBound])
            title = String(rest[colon.upperBound...])
        } else {
            category = ""
            title = rest
        }

        // Confidence footer.
        var confidence: Int?
        if let sub = text.range(of: "<sub>Fixloop · confidence ") {
            let tail = text[sub.upperBound...]
            confidence = Int(tail.prefix { $0.isNumber })
            text = String(text[..<sub.lowerBound])
        }

        // Suggestion block.
        var suggestion: String?
        if let start = text.range(of: "```suggestion\n") {
            if let end = text.range(of: "\n```", range: start.upperBound..<text.endIndex) {
                suggestion = String(text[start.upperBound..<end.lowerBound])
                text = String(text[..<start.lowerBound]) + String(text[end.upperBound...])
            }
        }

        // Fix line.
        var fix: String?
        if let f = text.range(of: "**Fix:** ") {
            let tail = text[f.upperBound...]
            let fixText = tail.components(separatedBy: "\n\n").first ?? String(tail)
            fix = fixText.trimmingCharacters(in: .whitespacesAndNewlines)
            text = String(text[..<f.lowerBound])
        }

        let rationale = text.trimmingCharacters(in: .whitespacesAndNewlines)
        return Finding(
            id: id, severity: severity, category: category, title: title, rationale: rationale,
            fix: fix, suggestion: suggestion, confidence: confidence, path: path, line: line, url: url
        )
    }
}
