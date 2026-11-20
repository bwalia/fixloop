import XCTest
@testable import Fixloop

final class ReviewParserTests: XCTestCase {
    // Real bodies posted by the Fixloop reviewer on bwalia/fixloop-test#1.
    let summary = """
    ### Fixloop review

    1 critical, 1 high.

    | Severity | Finding | Location |
    |---|---|---|
    | 🔴 critical | SQL injection via f-string in find_user | `app/users.py:10` |
    | 🟠 high | last() indexes out of bounds: items[items.len()] | `src/util.rs:7` |

    <sub>Model: claude · 3541 tokens · comment <code>/fixloop review</code> to run again</sub>
    """

    let comment = """
    **🔴 critical · security: SQL injection via f-string in find_user**

    The `name` argument is interpolated directly into the SQL string. An input such as `' OR '1'='1` changes the query.

    **Fix:** Use a parameterized query: cur.execute("SELECT id, email FROM users WHERE name = ?", (name,)).

    ```suggestion
        cur.execute("SELECT id, email FROM users WHERE name = ?", (name,))
    ```

    <sub>Fixloop · confidence 98%</sub>
    """

    func testSummary() throws {
        let s = try XCTUnwrap(ReviewParser.parseSummary(summary))
        XCTAssertEqual(s.headline, "1 critical, 1 high.")
        XCTAssertEqual(s.counts, [.critical: 1, .high: 1])
        XCTAssertEqual(s.total, 2)
        XCTAssertEqual(s.worst, .critical)
        XCTAssertEqual(s.model, "claude")
    }

    func testCleanSummary() throws {
        let s = try XCTUnwrap(ReviewParser.parseSummary("### Fixloop review\n\nNo problems found.\n"))
        XCTAssertEqual(s.total, 0)
        XCTAssertNil(s.worst)
        XCTAssertNil(s.model)
    }

    func testNotASummary() {
        XCTAssertNil(ReviewParser.parseSummary("LGTM"))
    }

    func testComment() throws {
        let f = try XCTUnwrap(ReviewParser.parseComment(id: 1, body: comment, path: "app/users.py", line: 10, url: nil))
        XCTAssertEqual(f.severity, .critical)
        XCTAssertEqual(f.category, "security")
        XCTAssertEqual(f.title, "SQL injection via f-string in find_user")
        XCTAssertTrue(f.rationale.hasPrefix("The `name` argument"))
        XCTAssertFalse(f.rationale.contains("Fix:"))
        XCTAssertEqual(f.fix?.hasPrefix("Use a parameterized query"), true)
        XCTAssertEqual(f.suggestion, "    cur.execute(\"SELECT id, email FROM users WHERE name = ?\", (name,))")
        XCTAssertEqual(f.confidence, 98)
    }

    func testCommentWithoutFixOrSuggestion() throws {
        let body = "**⚪ info · style: Prefer early return**\n\nNested ifs are hard to read.\n\n<sub>Fixloop · confidence 61%</sub>"
        let f = try XCTUnwrap(ReviewParser.parseComment(id: 2, body: body, path: "a.rs", line: nil, url: nil))
        XCTAssertEqual(f.severity, .info)
        XCTAssertEqual(f.category, "style")
        XCTAssertNil(f.fix)
        XCTAssertNil(f.suggestion)
        XCTAssertEqual(f.rationale, "Nested ifs are hard to read.")
        XCTAssertEqual(f.confidence, 61)
    }

    func testTitleContainingSeparators() throws {
        let body = "**🟠 high · correctness: last() indexes out of bounds: items[items.len()]**\n\nOff by one.\n\n<sub>Fixloop · confidence 90%</sub>"
        let f = try XCTUnwrap(ReviewParser.parseComment(id: 3, body: body, path: "src/util.rs", line: 7, url: nil))
        XCTAssertEqual(f.category, "correctness")
        XCTAssertEqual(f.title, "last() indexes out of bounds: items[items.len()]")
    }

    func testHumanCommentIgnored() {
        XCTAssertNil(ReviewParser.parseComment(id: 4, body: "**Nit:** rename this", path: "a", line: 1, url: nil))
    }
}
