import {
  Fragment,
  type ReactNode,
  useRef,
  useState,
  useSyncExternalStore,
} from "react"
import { CheckIcon, ChevronDownIcon, CopyIcon } from "lucide-react"
import { OSIcon } from "@/components/os-icon"
import { buttonVariants } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { DOWNLOADS, PLATFORMS, type Download } from "@/lib/links"
import { cn } from "@/lib/utils"

/** The download for the system the page is read on, as far as the browser says. */
function offered(): Download {
  const agent = navigator.userAgent
  const platform = navigator.platform
  const arm = /aarch64|arm64|armv8/i.test(`${agent} ${platform}`)
  const of = (system: Download["platform"]) =>
    DOWNLOADS.find(
      (download) =>
        download.platform === system &&
        (!download.arch || (download.arch === "arm64") === arm)
    ) ?? DOWNLOADS.find((download) => download.platform === system)
  if (/Android/.test(agent)) return of("android") ?? DOWNLOADS[0]
  if (/Win/.test(platform)) return of("windows") ?? DOWNLOADS[0]
  if (/Linux/.test(platform)) return of("linux") ?? DOWNLOADS[0]
  return DOWNLOADS[0]
}

/** The system never changes while the page is open. */
const unchanging = () => () => {}

/** Whether the install command was just copied, for a moment. */
function useCopied() {
  const [copied, setCopied] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)
  const copy = async (command: string) => {
    await navigator.clipboard.writeText(command)
    setCopied(true)
    clearTimeout(timer.current)
    timer.current = setTimeout(() => setCopied(false), 2000)
  }
  return { copied, copy }
}

/** The install command in a field of its own, copied by the button at its end. */
function CommandField({
  command,
  copied,
  copy,
}: {
  command: string
  copied: boolean
  copy: (command: string) => void
}) {
  return (
    <div className="flex max-w-full items-center gap-3 rounded-full border bg-muted py-1 pr-1 pl-5">
      <code className="overflow-x-auto font-mono text-sm whitespace-nowrap">
        <span className="text-muted-foreground select-none">$ </span>
        {command}
      </code>
      <button
        type="button"
        aria-label="Copy the install command"
        onClick={() => copy(command)}
        className={buttonVariants({ variant: "ghost", size: "icon-sm" })}
      >
        {copied ? <CheckIcon /> : <CopyIcon />}
      </button>
    </div>
  )
}

/** Downloads the app for the reader's system, with every other one behind the chevron, beside what the page passes in. An app installed by a command shows it above, in a field. */
export function DownloadButton({ children }: { children?: ReactNode }) {
  const primary = useSyncExternalStore(unchanging, offered, () => DOWNLOADS[0])
  const system = PLATFORMS.find(({ platform }) => platform === primary.platform)
  const { copied, copy } = useCopied()
  return (
    <div className="mt-8 flex flex-col items-center gap-4">
      {primary.command && (
        <CommandField command={primary.command} copied={copied} copy={copy} />
      )}
      <div className="flex flex-wrap justify-center gap-3">
        <div className="flex">
          {primary.url && (
            <a
              href={primary.url}
              className={cn(
                buttonVariants({ size: "lg" }),
                "rounded-r-none pr-3.5"
              )}
            >
              <OSIcon os={primary.platform} />
              Download for {system?.name}
            </a>
          )}
          <DropdownMenu>
            <DropdownMenuTrigger
              aria-label="Every download"
              className={
                primary.url
                  ? cn(
                      buttonVariants({ size: "icon-lg" }),
                      "w-9 rounded-l-none border-l-background/[calc(var(--opacity-color-tint-stronger)*100%)]"
                    )
                  : buttonVariants({ size: "lg", variant: "outline" })
              }
            >
              {!primary.url && "Other downloads"}
              <ChevronDownIcon />
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-72">
              {PLATFORMS.filter(
                ({ platform }) =>
                  !primary.command || platform !== primary.platform
              ).map(({ platform, name }, index) => (
                <Fragment key={platform}>
                  {index > 0 && <DropdownMenuSeparator />}
                  <DropdownMenuGroup>
                    <DropdownMenuLabel>{name}</DropdownMenuLabel>
                    {DOWNLOADS.filter(
                      (download) => download.platform === platform
                    ).map(({ title, detail, url, command }) => (
                      <DropdownMenuItem
                        key={title}
                        render={command ? undefined : <a href={url} />}
                        closeOnClick={!command}
                        onClick={() => command && copy(command)}
                        className="items-start"
                      >
                        <OSIcon os={platform} className="mt-0.5" />
                        <span className="flex flex-col gap-0.5">
                          {title}
                          <span className="text-xs font-normal text-muted-foreground">
                            {command && copied
                              ? "Copied, paste it in a terminal"
                              : detail}
                          </span>
                        </span>
                      </DropdownMenuItem>
                    ))}
                  </DropdownMenuGroup>
                </Fragment>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
        {children}
      </div>
    </div>
  )
}
