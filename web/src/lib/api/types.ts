export interface User {
  id: string;
  username: string;
  display_name: string;
  is_admin: boolean;
}

export interface AuthResponse {
  user: User;
}
