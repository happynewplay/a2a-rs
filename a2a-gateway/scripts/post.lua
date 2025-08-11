-- Lua script for wrk to test POST requests

wrk.method = "POST"
wrk.body = '{"message": {"role": "user", "parts": [{"type": "text", "text": "Hello, world!"}]}}'
wrk.headers["Content-Type"] = "application/json"
wrk.headers["Accept"] = "application/json"

-- Optional: Add authentication header
-- wrk.headers["Authorization"] = "Bearer your-token-here"

function response(status, headers, body)
    if status ~= 200 and status ~= 201 then
        print("Error response: " .. status .. " - " .. body)
    end
end
