import Foundation

/// Sample reviews for trying the app without a GitHub account (also used by App Review).
/// The bodies are exactly what the Fixloop reviewer posts, so they go through the real parser.
enum SampleData {
    static let login = "sample"

    private static func pr(_ repo: String, _ n: Int, _ title: String, _ author: String, hoursAgo: Double, open: Bool = true, summary: String) -> ReviewedPR {
        let date = Date().addingTimeInterval(-hoursAgo * 3600)
        return ReviewedPR(
            repo: repo, number: n, title: title, author: author,
            url: URL(string: "https://github.com/\(repo)/pull/\(n)")!,
            updatedAt: date, isOpen: open,
            summary: ReviewParser.parseSummary(summary, submittedAt: date)!
        )
    }

    static let inbox: [ReviewedPR] = [
        pr("acme/payments-api", 412, "Add refund endpoint for partial captures", "maya-k", hoursAgo: 0.3, summary: """
        ### Fixloop review

        1 critical, 1 high, 1 medium.

        <sub>Model: claude-sonnet-5-5 · 8120 tokens · comment <code>/fixloop review</code> to run again</sub>
        """),
        pr("acme/web", 1288, "Cache product listings on the edge", "tom-r", hoursAgo: 3, summary: """
        ### Fixloop review

        1 high, 2 low.

        <sub>Model: gpt-5 · 5310 tokens · comment <code>/fixloop review</code> to run again</sub>
        """),
        pr("acme/infra", 97, "Bump ingress controller to 1.12", "sam-o", hoursAgo: 26, summary: """
        ### Fixloop review

        No problems found.

        <sub>Model: llama-4 (local) · 2204 tokens · comment <code>/fixloop review</code> to run again</sub>
        """),
        pr("acme/mobile", 640, "Retry failed uploads with backoff", "priya-d", hoursAgo: 50, open: false, summary: """
        ### Fixloop review

        1 medium, 1 info.

        <sub>Model: claude-sonnet-5-5 · 4087 tokens · comment <code>/fixloop review</code> to run again</sub>
        """),
    ]

    private static func f(_ id: Int, _ body: String, _ path: String, _ line: Int) -> Finding {
        ReviewParser.parseComment(id: id, body: body, path: path, line: line, url: nil)!
    }

    static func findings(for pr: ReviewedPR) -> [Finding] {
        switch pr.number {
        case 412: [
            f(1, """
            **🔴 critical · security: Refund amount is not checked against the captured amount**

            `amount` comes straight from the request body and is passed to `gateway.refund`. A caller can refund more than was captured, or a negative amount.

            **Fix:** Reject amounts that are not positive or exceed `capture.remaining()`.

            ```suggestion
                if amount <= 0 || amount > capture.remaining() {
                    return Err(ApiError::invalid("amount"));
                }
            ```

            <sub>Fixloop · confidence 96%</sub>
            """, "src/refunds/handler.rs", 58),
            f(2, """
            **🟠 high · correctness: Two concurrent refunds can both pass the balance check**

            The remaining balance is read and then updated in separate statements without a lock, so two requests can each refund the full remainder.

            **Fix:** Use `SELECT … FOR UPDATE` on the capture row inside the transaction.

            <sub>Fixloop · confidence 88%</sub>
            """, "src/refunds/store.rs", 31),
            f(3, """
            **🟡 medium · reliability: Gateway timeout is retried without an idempotency key**

            A retry after a timeout can issue the refund twice at the gateway.

            **Fix:** Pass the refund id as the gateway's `Idempotency-Key` header.

            <sub>Fixloop · confidence 81%</sub>
            """, "src/gateway/client.rs", 112),
        ]
        case 1288: [
            f(4, """
            **🟠 high · security: Cache key ignores the signed-in user**

            Listings include member-only prices, but the cache key is only the URL, so one member's prices can be served to everyone.

            **Fix:** Add `Vary: Cookie` or exclude member pricing from the cached response.

            <sub>Fixloop · confidence 91%</sub>
            """, "edge/cache.ts", 24),
            f(5, """
            **🔵 low · maintainability: Magic number for TTL**

            `300` appears in three places. A named constant makes the TTL easier to change.

            ```suggestion
            const LISTING_TTL_SECONDS = 300;
            ```

            <sub>Fixloop · confidence 74%</sub>
            """, "edge/cache.ts", 9),
            f(6, """
            **🔵 low · testing: No test covers a cache miss**

            The new tests only exercise the hit path.

            <sub>Fixloop · confidence 70%</sub>
            """, "edge/cache.test.ts", 1),
        ]
        case 640: [
            f(7, """
            **🟡 medium · reliability: Backoff has no upper bound**

            After many failures the delay grows past an hour, so uploads appear stuck.

            **Fix:** Cap the delay, for example at 5 minutes.

            <sub>Fixloop · confidence 84%</sub>
            """, "Sources/Upload/RetryPolicy.swift", 22),
            f(8, """
            **⚪ info · style: Prefer `Duration` over `TimeInterval`**

            `Task.sleep(for:)` reads more clearly with `Duration` values.

            <sub>Fixloop · confidence 60%</sub>
            """, "Sources/Upload/RetryPolicy.swift", 30),
        ]
        default: []
        }
    }
}
