import { Fragment, type ReactNode, useSyncExternalStore } from "react"
import { ChevronDownIcon } from "lucide-react"
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

/** Downloads the app for the reader's system, with every other one behind the chevron, beside what the page passes in. */
export function DownloadButton({ children }: { children?: ReactNode }) {
  const primary = useSyncExternalStore(unchanging, offered, () => DOWNLOADS[0])
  const system = PLATFORMS.find(({ platform }) => platform === primary.platform)
  return (
    <div className="mt-8 flex flex-wrap justify-center gap-3">
      <div className="flex">
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
        <DropdownMenu>
          <DropdownMenuTrigger
            aria-label="Every download"
            className={cn(
              buttonVariants({ size: "icon-lg" }),
              "w-9 rounded-l-none border-l-background/[calc(var(--opacity-color-tint-stronger)*100%)]"
            )}
          >
            <ChevronDownIcon />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-72">
            {PLATFORMS.map(({ platform, name }, index) => (
              <Fragment key={platform}>
                {index > 0 && <DropdownMenuSeparator />}
                <DropdownMenuGroup>
                  <DropdownMenuLabel>{name}</DropdownMenuLabel>
                  {DOWNLOADS.filter(
                    (download) => download.platform === platform
                  ).map(({ title, detail, url }) => (
                    <DropdownMenuItem
                      key={url}
                      render={<a href={url} />}
                      className="items-start"
                    >
                      <OSIcon os={platform} className="mt-0.5" />
                      <span className="flex flex-col gap-0.5">
                        {title}
                        <span className="text-xs font-normal text-muted-foreground">
                          {detail}
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
  )
}
