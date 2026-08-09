import { createBrowserRouter } from "react-router-dom";
import ProtectedRoute from "@/components/layout/protected";
import HomePage from "@/routes/home";
import LoginPage from "@/routes/login";
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
      { path: "settings/tokens", element: <TokensPage /> },
      { path: "settings/users", element: <UsersPage /> },
      { path: "settings/audit", element: <AuditPage /> },
      { path: "api-docs", element: <ApiDocsPage /> },
    ],
  },
]);
