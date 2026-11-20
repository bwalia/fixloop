import Foundation

/// GitHub OAuth device flow: https://docs.github.com/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app#using-the-device-flow-to-generate-a-user-access-token
struct DeviceFlow: Sendable {
    struct Code: Decodable, Sendable {
        let device_code: String
        let user_code: String
        let verification_uri: String
        let expires_in: Int
        let interval: Int
    }

    enum FlowError: LocalizedError {
        case expired, denied, notConfigured, server(String)

        var errorDescription: String? {
            switch self {
            case .expired: "The code expired. Try again."
            case .denied: "Access was denied on GitHub."
            case .notConfigured: "GitHub sign-in isn't set up in this build. Use a token instead."
            case .server(let m): m
            }
        }
    }

    let clientID: String

    static var configured: DeviceFlow? {
        let id = (Bundle.main.object(forInfoDictionaryKey: "FixloopGitHubClientID") as? String) ?? ""
        let trimmed = id.trimmingCharacters(in: .whitespaces)
        return trimmed.isEmpty || trimmed.hasPrefix("$(") ? nil : DeviceFlow(clientID: trimmed)
    }

    func start() async throws -> Code {
        let data = try await post("https://github.com/login/device/code", ["client_id": clientID])
        if let err = try? JSONDecoder().decode(ErrorBody.self, from: data), let e = err.error {
            throw FlowError.server(err.error_description ?? e)
        }
        return try JSONDecoder().decode(Code.self, from: data)
    }

    func poll(_ code: Code) async throws -> String {
        var interval = max(code.interval, 5)
        let deadline = Date().addingTimeInterval(TimeInterval(code.expires_in))
        while Date() < deadline {
            try await Task.sleep(for: .seconds(interval))
            let data = try await post("https://github.com/login/oauth/access_token", [
                "client_id": clientID,
                "device_code": code.device_code,
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
            ])
            let body = try JSONDecoder().decode(TokenBody.self, from: data)
            if let token = body.access_token { return token }
            switch body.error {
            case "authorization_pending": continue
            case "slow_down": interval += 5
            case "expired_token": throw FlowError.expired
            case "access_denied": throw FlowError.denied
            default: throw FlowError.server(body.error_description ?? body.error ?? "Unknown error")
            }
        }
        throw FlowError.expired
    }

    private struct ErrorBody: Decodable { let error: String?; let error_description: String? }
    private struct TokenBody: Decodable { let access_token: String?; let error: String?; let error_description: String? }

    private func post(_ url: String, _ form: [String: String]) async throws -> Data {
        var req = URLRequest(url: URL(string: url)!)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Accept")
        req.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        var comps = URLComponents()
        comps.queryItems = form.map { URLQueryItem(name: $0.key, value: $0.value) }
        req.httpBody = comps.percentEncodedQuery?.data(using: .utf8)
        let (data, _) = try await ephemeralSession.data(for: req)
        return data
    }
}
