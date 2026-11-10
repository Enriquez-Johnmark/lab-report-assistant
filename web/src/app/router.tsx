import { createBrowserRouter } from "react-router"

import { AppShell } from "@/app/app-shell"
import { NotFoundPage } from "@/app/not-found-page"
import { PageLoading } from "@/app/page-loading"

/** Each page is loaded when first visited, so the first screen stays small. */
export const router = createBrowserRouter([
  {
    path: "/login",
    HydrateFallback: PageLoading,
    lazy: async () => ({ Component: (await import("@/features/login/login-page")).LoginPage }),
  },
  {
    path: "/login/:token",
    HydrateFallback: PageLoading,
    lazy: async () => ({
      Component: (await import("@/features/login/login-link-page")).LoginLinkPage,
    }),
  },
  {
    Component: AppShell,
    HydrateFallback: PageLoading,
    children: [
      {
        index: true,
        lazy: async () => ({
          Component: (await import("@/features/reports/reports-page")).ReportsPage,
        }),
      },
      {
        path: "sent",
        lazy: async () => ({
          Component: (await import("@/features/reports/reports-page")).ReportsPage,
        }),
      },
      {
        path: "reports/:id",
        lazy: async () => ({
          Component: (await import("@/features/reports/report-page")).ReportPage,
        }),
      },
      {
        path: "activity",
        lazy: async () => ({
          Component: (await import("@/features/activity/activity-page")).ActivityPage,
        }),
      },
      { path: "*", Component: NotFoundPage },
    ],
  },
])
