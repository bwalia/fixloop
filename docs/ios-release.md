# Releasing the Fixloop iOS app

The app lives in [`ios/`](../ios). It is a native SwiftUI app that shows the reviews the Fixloop reviewer has posted on GitHub pull requests. It talks only to GitHub; see the [privacy policy](https://fixloop.run/privacy/).

The release process follows the one used for the Fishers and GullyCricket apps: CI on every change, TestFlight first, then the build that was tested is promoted to the App Store. It is not rebuilt.

| What | How | When |
|---|---|---|
| Build + unit tests | `.github/workflows/ios.yml` | every PR and push touching `ios/**` |
| Listing readiness | **ios-release** → `store_status` | before anything is submitted |
| TestFlight | **ios-release** → `testflight` | to get a build to testers |
| Listing text | **ios-release** → `metadata` | when `ios/fastlane/metadata/` changes |
| Screenshots | **ios-release** → `screenshots` | when `ios/fastlane/screenshots/` changes |
| App Store review | **ios-release** → `app_store` | after TestFlight sign-off |

## Safety rules

- **Manual only.** `ios-release` never runs on push. Every run needs approval on the `app-store` environment.
- **GitHub-hosted runners only.** This repository is public and accepts outside contributions, so release jobs never run on a self-hosted machine.
- **One secret, no certificates.** The only signing material is the App Store Connect API key, stored on the `app-store` environment. Xcode manages the distribution certificate in the cloud (`-allowProvisioningUpdates`), so no `.p12` or certificate private key is ever exported. The key file is written to the runner's temp folder and deleted at the end of the job.
- **Promote, don't rebuild.** `app_store` attaches the newest processed TestFlight build and submits it. Release after approval is manual.
- **No placeholders.** `metadata` refuses to run while any value in `ios/fastlane/metadata/` is still in `[brackets]`.

## One-time setup

### 1. Create the app on Apple (in a browser, once)

The pipeline cannot do this: fastlane's `produce` does not accept an API key.

1. [developer.apple.com → Identifiers](https://developer.apple.com/account/resources/identifiers/list) → **+** → App IDs → App.
   Description `Fixloop`, Bundle ID **Explicit** `run.fixloop.app`. No extra capabilities are needed. Register.
2. [App Store Connect → Apps](https://appstoreconnect.apple.com/apps) → **+** → New App.
   Platform iOS, Name `Fixloop: AI Code Review`, Primary language English (U.K.), Bundle ID `run.fixloop.app`, SKU `run.fixloop.app`. Create.

### 2. Add the API key to the `app-store` environment

Settings → Environments → **New environment** `app-store`:

- **Required reviewers:** yourself.
- **Deployment branches:** `main` only.
- **Secrets:**
  - `ASC_KEY_ID`: the key id (the same team key Fishers uses works; the key is per team, not per app)
  - `ASC_ISSUER_ID`: the issuer id
  - `ASC_PRIVATE_KEY_B64`: `base64 -i AuthKey_<id>.p8 | pbcopy`

The key must have the **Admin** role for cloud-managed signing. If it is App Manager, `testflight` fails at export with a signing error.

### 3. Fill in the placeholders

In `ios/fastlane/metadata/`:

- `copyright.txt`: the year and the legal entity that publishes the app.
- `review_information/first_name.txt`, `last_name.txt`, `phone_number.txt`, `email_address.txt`: who Apple should contact.

No demo account is needed. The review notes tell App Review to use **Try with sample reviews**.

### 4. Set by hand in App Store Connect

The API cannot set these:

- **App Privacy:** Data Not Collected. This must stay true to the [privacy policy](https://fixloop.run/privacy/).
- **Age Rating:** answer the questionnaire (no objectionable content; the app opens github.com links in Safari).

## Releasing

1. **ios-release → `store_status`** shows what is still missing.
2. **ios-release → `testflight`** uploads a build. Install it from TestFlight and test it.
3. **ios-release → `metadata`**, then **`screenshots`**.
4. **ios-release → `store_status`** again, until only the two by-hand items remain.
5. **ios-release → `app_store`** submits for review.

To ship a new version, bump `MARKETING_VERSION` in `ios/project.yml`, merge, and run the steps again. Build numbers are worked out from TestFlight.

## Screenshots

`ios/fastlane/screenshots/en-GB/` holds 6.9" iPhone screenshots (1320×2868), taken by the UI test `FixloopUITests/ScreenshotTests` in sample mode:

```sh
cd ios && xcodegen generate
xcodebuild test -project Fixloop.xcodeproj -scheme Fixloop -only-testing:FixloopUITests \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro Max'
```

## Sign in with GitHub

Without configuration the app offers sign-in with a personal access token, plus sample mode. To enable **Sign in with GitHub** (OAuth device flow), set `FIXLOOP_GITHUB_CLIENT_ID` in `ios/Config.xcconfig` to the Client ID of the Fixloop GitHub App, and tick **Enable Device Flow** in that app's settings. The client ID is public; no client secret is used.
