export interface HttpRequest {
  url: string;
  method?: string;
  headers?: Record<string, string>;
  body?: string;
}
