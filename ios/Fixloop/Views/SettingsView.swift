import SwiftUI
import UserNotifications

struct SettingsView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @AppStorage("notify") private var notify = false
    @State private var confirmSignOut = false

    var body: some View {
        NavigationStack {
            Form {
                if model.isSample {
                    Section {
                        Button("Leave sample mode and sign in") {
                            model.signOut()
                            dismiss()
                        }
                    } footer: {
                        Text("You're looking at sample reviews. Sign in with GitHub to see Fixloop's reviews of your pull requests.")
                    }
                } else if case .signedIn(let login) = model.phase {
                    Section("GitHub account") {
                        LabeledContent("Signed in as", value: login.isEmpty ? "GitHub user" : login)
                        Button("Sign out", role: .destructive) { confirmSignOut = true }
                    }
                }

                Section {
                    Toggle("Notify me about new reviews", isOn: $notify)
                        .onChange(of: notify) { _, on in
                            guard on else { return }
                            Task {
                                notify = await BackgroundRefresh.requestNotificationPermission()
                                if notify { BackgroundRefresh.schedule() }
                            }
                        }
                } footer: {
                    Text("iOS checks GitHub in the background from time to time. How often depends on how you use the app.")
                }

                Section("Help") {
                    Link(destination: Brand.support) {
                        LabeledContent("Email support", value: "support@workstation.co.uk")
                    }
                    Link("Fixloop website", destination: Brand.website)
                    Link("Privacy policy", destination: Brand.privacy)
                    Link("Source code", destination: Brand.source)
                }

                Section {
                    LabeledContent("Version", value: Bundle.main.appVersion)
                } footer: {
                    Text("Fixloop is open source under the Apache License 2.0. Sponsored by Workstation AI.")
                }
            }
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
            }
            .confirmationDialog("Sign out of GitHub?", isPresented: $confirmSignOut, titleVisibility: .visible) {
                Button("Sign out", role: .destructive) {
                    model.signOut()
                    dismiss()
                }
            } message: {
                Text("Your token is removed from this device.")
            }
        }
    }
}

extension Bundle {
    var appVersion: String {
        let v = object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "?"
        let b = object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "?"
        return "\(v) (\(b))"
    }
}
