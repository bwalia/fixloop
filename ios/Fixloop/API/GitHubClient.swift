import Foundation

/// Ephemeral session: GitHub responses and cookies are kept in memory only, never written to disk.
let ephemeralSession = URLSession(configuration: .ephemeral)

enum GitHubError: LocalizedError {
    case unauthorized
    case rateLimited
    case http(Int, String)

    var errorDescription: String? {
        switch self {
        case .unauthorized: "Your GitHub sign-in has expired. Sign in again."
        case .rateLimited: "GitHub's rate limit was reached. Try again in a few minutes."
        case .http(let code, let msg): "GitHub returned \(code): \(msg)"
        }
    }
}

/// Minimal GitHub REST client for the calls the app needs.
struct GitHubClient: Sendable {
    let token: String
    var base = URL(string: "https://api.github.com")!

    private static let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.dateDecodingStrategy = .iso8601
        return d
    }()

    // MARK: Wire types

    struct User: Decodable, Sendable { let login: String; let avatar_url: String? }

    struct SearchResult: Decodable, Sendable { let items: [Issue] }
    struct Issue: Decodable, Sendable {
        let number: Int
        let title: String
        let html_url: URL
        let state: String
        let updated_at: Date
        let repository_url: URL
        let user: User
    }

    struct Review: Decodable, Sendable {
        let id: Int
        let user: User?
        let body: String?
        let submitted_at: Date?
    }

    struct ReviewComment: Decodable, Sendable {
        let id: Int
        let user: User?
        let body: String
        let path: String
        let line: Int?
        let original_line: Int?
        let html_url: URL?
    }

    // MARK: Calls

    func viewer() async throws -> User {
        try await get("/user")
    }

    /// Pull requests the user is involved in, most recently updated first.
    func involvedPullRequests(includeClosed: Bool) async throws -> [Issue] {
        var q = "is:pr involves:@me archived:false"
        if !includeClosed { q += " is:open" }
        let result: SearchResult = try await get("/search/issues", query: [
            "q": q, "sort": "updated", "order": "desc", "per_page": "40",
        ])
        return result.items
    }

    func reviews(repo: String, number: Int) async throws -> [Review] {
        try await get("/repos/\(repo)/pulls/\(number)/reviews", query: ["per_page": "100"])
    }

    func reviewComments(repo: String, number: Int) async throws -> [ReviewComment] {
        try await get("/repos/\(repo)/pulls/\(number)/comments", query: ["per_page": "100"])
    }

    // MARK: Plumbing

    private func get<T: Decodable>(_ path: String, query: [String: String] = [:]) async throws -> T {
        var comps = URLComponents(url: base.appendingPathComponent(path), resolvingAgainstBaseURL: false)!
        if !query.isEmpty {
            comps.queryItems = query.sorted { $0.key < $1.key }.map { URLQueryItem(name: $0.key, value: $0.value) }
        }
        var req = URLRequest(url: comps.url!)
        req.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        req.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        req.setValue("2022-11-28", forHTTPHeaderField: "X-GitHub-Api-Version")
        let (data, response) = try await ephemeralSession.data(for: req)
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        switch status {
        case 200..<300:
            return try Self.decoder.decode(T.self, from: data)
        case 401:
            throw GitHubError.unauthorized
        case 429:
            throw GitHubError.rateLimited
        case 403 where (response as? HTTPURLResponse)?.value(forHTTPHeaderField: "x-ratelimit-remaining") == "0":
            throw GitHubError.rateLimited
        default:
            let msg = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["message"] as? String
            throw GitHubError.http(status, msg ?? "request failed")
        }
    }
}

extension GitHubClient.Issue {
    /// "owner/name" from the repository URL.
    var repoFullName: String {
        let parts = repository_url.pathComponents
        return parts.suffix(2).joined(separator: "/")
    }
}
