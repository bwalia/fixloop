import SwiftUI

struct SignInView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openURL) private var openURL

    @State private var code: DeviceFlow.Code?
    @State private var flowTask: Task<Void, Never>?
    @State private var showToken = false
    @State private var token = ""
    @State private var busy = false
    @State private var error: String?

    private let flow = DeviceFlow.configured

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 28) {
                    LogoView()
                        .frame(width: 88, height: 88)
                        .padding(.top, 40)

                    VStack(spacing: 10) {
                        Text("Fixloop")
                            .font(.largeTitle.bold())
                        Text("See what Fixloop found in your pull requests: every finding, its severity and the suggested fix, in one inbox.")
                            .multilineTextAlignment(.center)
                            .foregroundStyle(.secondary)
                    }

                    if let code {
                        deviceCodeCard(code)
                    } else {
                        actions
                    }

                    if let error {
                        Text(error)
                            .font(.footnote)
                            .foregroundStyle(.red)
                            .multilineTextAlignment(.center)
                    }

                    Text("Your GitHub token stays on this device. Fixloop for iOS talks only to GitHub.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                }
                .padding(.horizontal, 24)
                .frame(maxWidth: 520)
                .frame(maxWidth: .infinity)
            }
            .sheet(isPresented: $showToken) { tokenSheet }
        }
    }

    private var actions: some View {
        VStack(spacing: 12) {
            if flow != nil {
                Button {
                    startDeviceFlow()
                } label: {
                    Label("Sign in with GitHub", systemImage: "person.badge.key")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(busy)
            }
            Button {
                showToken = true
            } label: {
                Text(flow == nil ? "Sign in with a GitHub token" : "Use a personal access token")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(flow == nil ? AnyButtonStyle(.borderedProminent) : AnyButtonStyle(.bordered))
            .controlSize(.large)
            Button("Try with sample reviews") { model.useSampleData() }
                .padding(.top, 4)
        }
    }

    private func deviceCodeCard(_ code: DeviceFlow.Code) -> some View {
        VStack(spacing: 16) {
            Text("Enter this code on GitHub")
                .font(.headline)
            Text(code.user_code)
                .font(.system(.largeTitle, design: .monospaced).weight(.semibold))
                .textSelection(.enabled)
                .accessibilityLabel("Code \(code.user_code.map(String.init).joined(separator: " "))")
            Button {
                UIPasteboard.general.string = code.user_code
                if let url = URL(string: code.verification_uri) { openURL(url) }
            } label: {
                Label("Copy code and open GitHub", systemImage: "arrow.up.forward.app")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            HStack(spacing: 8) {
                ProgressView()
                Text("Waiting for you to approve…").foregroundStyle(.secondary)
            }
            Button("Cancel", role: .cancel) {
                flowTask?.cancel()
                self.code = nil
            }
        }
        .padding(20)
        .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 16))
    }

    private var tokenSheet: some View {
        NavigationStack {
            Form {
                Section {
                    SecureField("ghp_… or github_pat_…", text: $token)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .font(.system(.body, design: .monospaced))
                } footer: {
                    Text("Create a fine-grained token with read-only access to Pull requests on the repositories Fixloop reviews. It is stored in the iOS keychain on this device.")
                }
                Section {
                    Link("Create a token on GitHub", destination: URL(string: "https://github.com/settings/personal-access-tokens/new")!)
                }
            }
            .navigationTitle("GitHub token")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { showToken = false } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Sign in") {
                        Task {
                            busy = true
                            defer { busy = false }
                            do {
                                try await model.signIn(token: token)
                                showToken = false
                            } catch {
                                self.error = error.localizedDescription
                                showToken = false
                            }
                        }
                    }
                    .disabled(token.trimmingCharacters(in: .whitespaces).isEmpty || busy)
                }
            }
        }
        .presentationDetents([.medium, .large])
    }

    private func startDeviceFlow() {
        guard let flow else { return }
        error = nil
        busy = true
        flowTask = Task {
            defer { busy = false }
            do {
                let c = try await flow.start()
                code = c
                let token = try await flow.poll(c)
                try await model.signIn(token: token)
            } catch is CancellationError {
            } catch {
                self.error = error.localizedDescription
            }
            code = nil
        }
    }
}

/// Lets one button switch style at runtime.
struct AnyButtonStyle: PrimitiveButtonStyle {
    private let make: (Configuration) -> AnyView
    init<S: PrimitiveButtonStyle>(_ style: S) { make = { AnyView(style.makeBody(configuration: $0)) } }
    func makeBody(configuration: Configuration) -> some View { make(configuration) }
}
