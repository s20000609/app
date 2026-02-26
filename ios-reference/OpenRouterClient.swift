//
// OpenRouterClient.swift
// 精簡範例：打 OpenRouter（OpenAI 相容）API，支援 stream。
// 直接加入 Xcode 專案，與 PromptEngine 搭配使用。
//

import Foundation

/// OpenAI 相容的 chat message
public struct ChatMessage: Encodable {
    public var role: String  // "system" | "user" | "assistant"
    public var content: String

    public init(role: String, content: String) {
        self.role = role
        self.content = content
    }
}

/// 請求 body（OpenAI 格式）
private struct ChatRequest: Encodable {
    let model: String
    let messages: [ChatMessage]
    let stream: Bool
    let temperature: Double?
    let max_tokens: Int?

    enum CodingKeys: String, CodingKey {
        case model, messages, stream, temperature
        case max_tokens = "max_tokens"
    }
}

/// SSE 解析：取 data: 行內容，若為 [DONE] 則結束
private func parseSSELine(_ line: String) -> String? {
    let trimmed = line.trimmingCharacters(in: .whitespaces)
    guard trimmed.hasPrefix("data: ") else { return nil }
    let payload = String(trimmed.dropFirst(6)).trimmingCharacters(in: .whitespaces)
    if payload == "[DONE]" { return nil }
    return payload
}

/// 從 data 行 JSON 取出 choices[0].delta.content
private func extractDeltaContent(from jsonData: Data) -> String? {
    guard let json = try? JSONSerialization.jsonObject(with: jsonData) as? [String: Any],
          let choices = json["choices"] as? [[String: Any]],
          let first = choices.first,
          let delta = first["delta"] as? [String: Any],
          let content = delta["content"] as? String else { return nil }
    return content
}

public final class OpenRouterClient {
    public static let defaultURL = URL(string: "https://openrouter.ai/api/v1/chat/completions")!
    private let session: URLSession
    private let baseURL: URL

    public init(session: URLSession = .shared, baseURL: URL = defaultURL) {
        self.session = session
        self.baseURL = baseURL
    }

    /// 非串流：發送後等完整回覆。
    public func send(
        messages: [ChatMessage],
        systemPrompt: String? = nil,
        apiKey: String,
        model: String = "openai/gpt-3.5-turbo",
        temperature: Double = 0.7,
        maxTokens: Int = 1024
    ) async throws -> String {
        var msgs = messages
        if let sys = systemPrompt, !sys.isEmpty {
            msgs.insert(ChatMessage(role: "system", content: sys), at: 0)
        }
        let body = ChatRequest(model: model, messages: msgs, stream: false, temperature: temperature, max_tokens: maxTokens)
        var req = URLRequest(url: baseURL)
        req.httpMethod = "POST"
        req.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = try JSONEncoder().encode(body)

        let (data, response) = try await session.data(for: req)
        guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
            throw NSError(domain: "OpenRouterClient", code: -1, userInfo: [NSLocalizedDescriptionKey: String(data: data, encoding: .utf8) ?? "Unknown error"])
        }
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let choices = json["choices"] as? [[String: Any]],
              let first = choices.first,
              let message = first["message"] as? [String: Any],
              let content = message["content"] as? String else {
            throw NSError(domain: "OpenRouterClient", code: -2, userInfo: [NSLocalizedDescriptionKey: "Invalid response"])
        }
        return content
    }

    /// 串流：一邊收 SSE 一邊回呼 onDelta，完成後呼叫 completion。
    public func sendStream(
        messages: [ChatMessage],
        systemPrompt: String? = nil,
        apiKey: String,
        model: String = "openai/gpt-3.5-turbo",
        temperature: Double = 0.7,
        maxTokens: Int = 1024,
        onDelta: @escaping (String) -> Void,
        completion: @escaping (Result<Void, Error>) -> Void
    ) {
        var msgs = messages
        if let sys = systemPrompt, !sys.isEmpty {
            msgs.insert(ChatMessage(role: "system", content: sys), at: 0)
        }
        let body = ChatRequest(model: model, messages: msgs, stream: true, temperature: temperature, max_tokens: maxTokens)
        var req = URLRequest(url: baseURL)
        req.httpMethod = "POST"
        req.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        req.httpBody = try? JSONEncoder().encode(body)

        let task = session.dataTask(with: req) { data, response, error in
            if let error = error {
                completion(.failure(error))
                return
            }
            guard let data = data else {
                completion(.failure(NSError(domain: "OpenRouterClient", code: -1, userInfo: [NSLocalizedDescriptionKey: "No data"])))
                return
            }
            let lines = String(data: data, encoding: .utf8)?.components(separatedBy: .newlines) ?? []
            for line in lines {
                if let payload = parseSSELine(line), let payloadData = payload.data(using: .utf8), let content = extractDeltaContent(from: payloadData), !content.isEmpty {
                    DispatchQueue.main.async { onDelta(content) }
                }
            }
            completion(.success(()))
        }
        task.resume()
    }
}
