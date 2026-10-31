export const GITHUB_URL = "https://github.com/yekta/wonnet"
export const GETTING_STARTED_URL = `${GITHUB_URL}#getting-started`
export const CONTACT_EMAIL = "wonnetapp@gmail.com"
export const TAGLINE = "Email that feels instant"
export const DESCRIPTION =
  "A calm, plain mail app for the Mac, iPhone, iPad, Windows, Linux and Android. Everything you do happens on your device at once; the network catches up in the background."

const LATEST_RELEASE = `${GITHUB_URL}/releases/latest/download`

/** A system an app is built for. */
export type Platform = "macos" | "windows" | "linux" | "android"

export const PLATFORMS: { platform: Platform; name: string }[] = [
  { platform: "macos", name: "macOS" },
  { platform: "windows", name: "Windows" },
  { platform: "linux", name: "Linux" },
  { platform: "android", name: "Android" },
]

export type Download = {
  platform: Platform
  /** The processor it is built for, where the system has more than one. */
  arch?: "x86_64" | "arm64"
  title: string
  detail: string
  url: string
}

/** Every app a release carries; the first of a system is the one it is offered. */
export const DOWNLOADS: Download[] = [
  {
    platform: "macos",
    title: "Apple Silicon",
    detail: "macOS 14 or later. It updates itself",
    url: `${LATEST_RELEASE}/Wonnet.zip`,
  },
  {
    platform: "windows",
    title: "Intel and AMD",
    detail: "Not for ARM PCs. It updates itself",
    url: `${LATEST_RELEASE}/Wonnet-windows-x86_64.zip`,
  },
  {
    platform: "linux",
    arch: "x86_64",
    title: "Intel and AMD",
    detail: "AppImage. It updates itself",
    url: `${LATEST_RELEASE}/Wonnet-linux-x86_64.AppImage`,
  },
  {
    platform: "linux",
    arch: "arm64",
    title: "ARM",
    detail: "AppImage. It updates itself",
    url: `${LATEST_RELEASE}/Wonnet-linux-aarch64.AppImage`,
  },
  {
    platform: "android",
    title: "Phones and tablets",
    detail: "Android 10 or later. It updates itself",
    url: `${LATEST_RELEASE}/Wonnet.apk`,
  },
]
