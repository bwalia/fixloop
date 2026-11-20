import Foundation
import Observation

/// App-wide state: sign-in and the review inbox.
@MainActor
@Observable
final class AppModel {
    enum Phase: Equatable { case signedOut, signedIn(login: String) }

    private(set) var phase: Phase = .signedOut
    private(set) var inbox: [ReviewedPR] = []
    private(set) var isLoading = false
    var errorMessage: String?
    var includeClosed = false {
        didSet { if oldValue != includeClosed { Task { await refresh() } } }
    }

    private var token: String?
    private(set) var isSample = false

    init() {
        let args = ProcessInfo.processInfo.arguments
        if args.contains("-sampleData") {
            useSampleData()
        } else if let saved = Keychain.load() {
            token = saved
            phase = .signedIn(login: UserDefaults.standard.string(forKey: "login") ?? "")
        }
    }

    var client: GitHubClient? { token.map { GitHubClient(token: $0) } }

    // MARK: Auth

    func signIn(token raw: String) async throws {
        let token = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        let user = try await GitHubClient(token: token).viewer()
        Keychain.save(token)
        UserDefaults.standard.set(user.login, forKey: "login")
        self.token = token
        phase = .signedIn(login: user.login)
        await refresh()
    }

    /// Try the app with built-in sample reviews; nothing is sent anywhere.
    func useSampleData() {
        isSample = true
        inbox = SampleData.inbox
        phase = .signedIn(login: SampleData.login)
    }

    func findings(for pr: ReviewedPR) async throws -> [Finding] {
        if isSample { return SampleData.findings(for: pr) }
        guard let client else { return [] }
        return try await InboxLoader(client: client).findings(for: pr)
    }

    func signOut() {
        isSample = false
        Keychain.delete()
        UserDefaults.standard.removeObject(forKey: "login")
        SeenStore.reset()
        token = nil
        inbox = []
        phase = .signedOut
    }

    // MARK: Inbox

    func refresh() async {
        if isSample {
            inbox = includeClosed ? SampleData.inbox : SampleData.inbox.filter(\.isOpen)
            return
        }
        guard let client else { return }
        isLoading = true
        defer { isLoading = false }
        do {
            inbox = try await InboxLoader(client: client).load(includeClosed: includeClosed)
            SeenStore.markSeen(inbox)
            errorMessage = nil
        } catch GitHubError.unauthorized {
            signOut()
            errorMessage = GitHubError.unauthorized.errorDescription
        } catch is CancellationError {
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

/// Loads pull requests that have a Fixloop review. Shared by the UI and background refresh.
struct InboxLoader: Sendable {
    let client: GitHubClient

    func load(includeClosed: Bool) async throws -> [ReviewedPR] {
        let issues = try await client.involvedPullRequests(includeClosed: includeClosed)
        return try await withThrowingTaskGroup(of: ReviewedPR?.self) { group in
            for issue in issues {
                group.addTask {
                    let repo = issue.repoFullName
                    let reviews = try await client.reviews(repo: repo, number: issue.number)
                    guard let latest = reviews.last(where: {
                        ($0.user?.login.hasSuffix("[bot]") ?? false) && ReviewParser.isFixloopSummary($0.body ?? "")
                    }), let summary = ReviewParser.parseSummary(latest.body ?? "", submittedAt: latest.submitted_at) else {
                        return nil
                    }
                    return ReviewedPR(
                        repo: repo, number: issue.number, title: issue.title, author: issue.user.login,
                        url: issue.html_url, updatedAt: latest.submitted_at ?? issue.updated_at,
                        isOpen: issue.state == "open", summary: summary
                    )
                }
            }
            var out: [ReviewedPR] = []
            for try await pr in group { if let pr { out.append(pr) } }
            return out.sorted { $0.updatedAt > $1.updatedAt }
        }
    }

    func findings(for pr: ReviewedPR) async throws -> [Finding] {
        let comments = try await client.reviewComments(repo: pr.repo, number: pr.number)
        return comments.compactMap { c in
            guard c.user?.login.hasSuffix("[bot]") ?? false else { return nil }
            return ReviewParser.parseComment(id: c.id, body: c.body, path: c.path, line: c.line ?? c.original_line, url: c.html_url)
        }
        .sorted { ($0.severity, $0.path, $0.line ?? 0) < ($1.severity, $1.path, $1.line ?? 0) }
    }
}

/// Remembers which reviews the user has already been told about.
enum SeenStore {
    private static let key = "seenReviews"

    static func key(for pr: ReviewedPR) -> String {
        "\(pr.id)@\(pr.summary.submittedAt?.timeIntervalSince1970 ?? 0)"
    }

    static func unseen(_ prs: [ReviewedPR]) -> [ReviewedPR] {
        let seen = Set(UserDefaults.standard.stringArray(forKey: key) ?? [])
        return prs.filter { !seen.contains(key(for: $0)) }
    }

    static func markSeen(_ prs: [ReviewedPR]) {
        var seen = UserDefaults.standard.stringArray(forKey: key) ?? []
        seen.append(contentsOf: prs.map(key(for:)))
        UserDefaults.standard.set(Array(Set(seen)).suffix(500), forKey: key)
    }

    static func reset() { UserDefaults.standard.removeObject(forKey: key) }
}
