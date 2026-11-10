import { useMutation } from "@tanstack/react-query"
import { FileTextIcon, HistoryIcon, LogOutIcon, SendIcon, type LucideIcon } from "lucide-react"
import { Navigate, NavLink, Outlet, useLocation } from "react-router"

import { api, ApiError } from "@/api/client"
import { useMe } from "@/app/queries"
import { Logo } from "@/components/logo"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar"
import { Skeleton } from "@/components/ui/skeleton"

type NavItem = { to: string; label: string; icon: LucideIcon }

const reports: NavItem = { to: "/", label: "Lab reports", icon: FileTextIcon }
const sent: NavItem = { to: "/sent", label: "Sent", icon: SendIcon }
const activity: NavItem = { to: "/activity", label: "Activity", icon: HistoryIcon }

/** The logged-in frame: navigation beside (or, on a phone, above) the page. */
export function AppShell() {
  const me = useMe()
  // Only an ended session means logging in again; anything else (a deploy, a
  // dropped connection) is retried while the person stays where they are.
  if (me.error instanceof ApiError && me.error.isNotLoggedIn)
    return <Navigate to="/login" replace />
  return (
    <SidebarProvider>
      <Sidebar>
        <SidebarHeader>
          <Logo className="px-2 py-1.5" />
        </SidebarHeader>
        <SidebarContent>
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarMenu>
                <NavMenuItem item={reports} />
                <NavMenuItem item={sent} />
                {me.data?.role === "administrator" ? <NavMenuItem item={activity} /> : null}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarContent>
        <SidebarFooter>
          <AccountMenu
            clinicName={me.data?.clinic_name}
            who={me.data ? (me.data.name ?? me.data.email) : undefined}
          />
        </SidebarFooter>
      </Sidebar>
      <SidebarInset>
        <header className="flex h-12 items-center gap-2 border-b px-4 md:hidden">
          <SidebarTrigger />
          <Logo />
        </header>
        <main className="mx-auto flex w-full max-w-6xl flex-col gap-6 p-4 md:p-8">
          <Outlet />
        </main>
      </SidebarInset>
    </SidebarProvider>
  )
}

function NavMenuItem({ item }: { item: NavItem }) {
  const { pathname } = useLocation()
  const isActive = item.to === "/" ? pathname === "/" : pathname.startsWith(item.to)
  const Icon = item.icon
  return (
    <SidebarMenuItem>
      <SidebarMenuButton isActive={isActive} render={<NavLink to={item.to} />}>
        <Icon />
        <span>{item.label}</span>
      </SidebarMenuButton>
    </SidebarMenuItem>
  )
}

function AccountMenu({ clinicName, who }: { clinicName?: string; who?: string }) {
  const logOut = useMutation({
    mutationFn: () => api("POST", "/api/logout"),
    onSettled: () => location.assign("/login"),
  })
  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <div className="flex flex-col gap-0.5 px-2 py-1.5 text-sm">
          {clinicName ? (
            <span className="truncate font-medium">{clinicName}</span>
          ) : (
            <Skeleton className="h-4 w-32" />
          )}
          {who ? <span className="truncate text-xs text-muted-foreground">{who}</span> : null}
        </div>
      </SidebarMenuItem>
      <SidebarMenuItem>
        <SidebarMenuButton onClick={() => logOut.mutate()} disabled={logOut.isPending}>
          <LogOutIcon />
          <span>Log out</span>
        </SidebarMenuButton>
      </SidebarMenuItem>
    </SidebarMenu>
  )
}
