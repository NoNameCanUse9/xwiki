import { createBrowserRouter } from "react-router-dom";
import ProtectedRoute from "@/components/layout/protected";
import HomePage from "@/routes/home";
import LoginPage from "@/routes/login";
import ProjectDetailPage from "@/routes/project-detail";

export const router = createBrowserRouter([
  { path: "/login", element: <LoginPage /> },
  {
    path: "/",
    element: <ProtectedRoute />,
    children: [
      { index: true, element: <HomePage /> },
      { path: "projects/:id", element: <ProjectDetailPage /> },
    ],
  },
]);
