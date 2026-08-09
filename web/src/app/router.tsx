import { createBrowserRouter } from "react-router-dom";
import ProtectedRoute from "@/components/layout/protected";
import AdminRoute from "@/components/layout/admin";
import HomePage from "@/routes/home";
import LoginPage from "@/routes/login";
import AccountPage from "@/routes/account";
import ProjectDetailPage from "@/routes/project-detail";
import DocsViewerPage from "@/routes/docs-viewer";
import TokensPage from "@/routes/tokens";
import ApiDocsPage from "@/routes/api-docs";
import UsersPage from "@/routes/users";
import AuditPage from "@/routes/audit";

export const router = createBrowserRouter([
  { path: "/login", element: <LoginPage /> },
  {
    path: "/",
    element: <ProtectedRoute />,
    children: [
      { index: true, element: <HomePage /> },
      { path: "projects/:id", element: <ProjectDetailPage /> },
      { path: "projects/:id/docs/*", element: <DocsViewerPage /> },
      {
        path: "settings",
        children: [
          { path: "account", element: <AccountPage /> },
          {
            element: <AdminRoute />,
            children: [
              { path: "tokens", element: <TokensPage /> },
              { path: "users", element: <UsersPage /> },
              { path: "audit", element: <AuditPage /> },
            ],
          },
        ],
      },
      { path: "api-docs", element: <ApiDocsPage /> },
    ],
  },
]);
