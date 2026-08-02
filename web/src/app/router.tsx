import { createBrowserRouter } from "react-router-dom";
import ProtectedRoute from "@/components/layout/protected";
import HomePage from "@/routes/home";
import LoginPage from "@/routes/login";

export const router = createBrowserRouter([
  { path: "/login", element: <LoginPage /> },
  {
    path: "/",
    element: <ProtectedRoute />,
    children: [{ index: true, element: <HomePage /> }],
  },
]);
