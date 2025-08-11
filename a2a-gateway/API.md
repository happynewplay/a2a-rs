# A2A Gateway API Documentation

The A2A Gateway provides both management APIs and proxies A2A protocol endpoints.

## Table of Contents

- [Authentication](#authentication)
- [Management APIs](#management-apis)
- [A2A Protocol Endpoints](#a2a-protocol-endpoints)
- [WebSocket Support](#websocket-support)
- [Error Handling](#error-handling)
- [Rate Limiting](#rate-limiting)

## Authentication

The gateway supports multiple authentication methods:

### Bearer Token

```http
Authorization: Bearer <token>
```

### API Key

```http
X-API-Key: <api-key>
```

### JWT

```http
Authorization: Bearer <jwt-token>
```

## Management APIs

### Health Check

Check gateway health status.

```http
GET /health
```

**Response:**

```json
{
  "status": "healthy",
  "timestamp": "2024-01-15T10:30:00Z",
  "services": 3,
  "components": {
    "service_discovery": {
      "status": "healthy",
      "message": "All services discovered"
    },
    "load_balancer": {
      "status": "healthy",
      "message": "Load balancing operational"
    }
  }
}
```

### List Services

Get all registered services.

```http
GET /services
```

**Response:**

```json
{
  "services": [
    {
      "id": "service-123",
      "name": "reimbursement-agent",
      "url": "http://localhost:3001",
      "status": "healthy",
      "weight": 100,
      "tags": ["finance", "reimbursement"],
      "last_health_check": "2024-01-15T10:29:00Z",
      "agent_card": {
        "name": "Reimbursement Agent",
        "description": "Handles expense reimbursements",
        "skills": [
          {
            "name": "process_reimbursement",
            "description": "Process expense reimbursement requests"
          }
        ]
      }
    }
  ],
  "count": 1
}
```

### Get Service

Get details of a specific service.

```http
GET /services/{service_id}
```

**Response:**

```json
{
  "id": "service-123",
  "name": "reimbursement-agent",
  "url": "http://localhost:3001",
  "status": "healthy",
  "weight": 100,
  "tags": ["finance", "reimbursement"],
  "metadata": {
    "version": "1.0.0",
    "region": "us-west-2"
  },
  "registered_at": "2024-01-15T09:00:00Z",
  "updated_at": "2024-01-15T10:29:00Z"
}
```

### Metrics

Get Prometheus metrics.

```http
GET /metrics
```

**Response:**

```
# HELP a2a_gateway_requests_total Total number of requests
# TYPE a2a_gateway_requests_total counter
a2a_gateway_requests_total{method="GET",status="200"} 1234

# HELP a2a_gateway_request_duration_seconds Request duration in seconds
# TYPE a2a_gateway_request_duration_seconds histogram
a2a_gateway_request_duration_seconds_bucket{le="0.1"} 100
a2a_gateway_request_duration_seconds_bucket{le="0.5"} 200
a2a_gateway_request_duration_seconds_sum 45.6
a2a_gateway_request_duration_seconds_count 250
```

### Reload Configuration

Reload gateway configuration.

```http
POST /reload
```

**Response:**

```json
{
  "status": "reloaded",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

## A2A Protocol Endpoints

The gateway proxies all A2A protocol endpoints to backend agents.

### Agent Card Discovery

Get agent capabilities.

```http
GET /.well-known/agent-card
```

**Response:**

```json
{
  "name": "A2A Gateway",
  "description": "Gateway for Agent-to-Agent communication",
  "version": "1.0.0",
  "skills": [
    {
      "name": "route_request",
      "description": "Route requests to appropriate agents"
    }
  ],
  "endpoints": {
    "tasks": "/tasks",
    "websocket": "/ws"
  }
}
```

### Send Task

Send a task to an agent.

```http
POST /tasks/send
Content-Type: application/json
```

**Request:**

```json
{
  "role": "user",
  "parts": [
    {
      "type": "text",
      "content": "Process my expense report for $150"
    }
  ],
  "metadata": {
    "skill": "process_reimbursement",
    "priority": "normal"
  }
}
```

**Response:**

```json
{
  "taskId": "task-456",
  "status": "accepted",
  "agent": "reimbursement-agent",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### Send Task with Streaming

Send a task with streaming response.

```http
POST /tasks/sendSubscribe
Content-Type: application/json
```

**Request:**

```json
{
  "role": "user",
  "parts": [
    {
      "type": "text",
      "content": "Generate a detailed financial report"
    }
  ],
  "metadata": {
    "skill": "generate_report",
    "stream": true
  }
}
```

**Response:** (Server-Sent Events)

```
data: {"type": "start", "taskId": "task-789"}

data: {"type": "progress", "content": "Analyzing financial data..."}

data: {"type": "progress", "content": "Generating charts..."}

data: {"type": "complete", "result": "Report generated successfully"}
```

### Get Task Status

Get the status of a task.

```http
GET /tasks/{task_id}
```

**Response:**

```json
{
  "taskId": "task-456",
  "status": "completed",
  "result": {
    "type": "text",
    "content": "Expense report processed. Reimbursement of $150 approved."
  },
  "agent": "reimbursement-agent",
  "created_at": "2024-01-15T10:30:00Z",
  "completed_at": "2024-01-15T10:31:30Z"
}
```

### Cancel Task

Cancel a running task.

```http
POST /tasks/{task_id}/cancel
```

**Response:**

```json
{
  "taskId": "task-456",
  "status": "cancelled",
  "timestamp": "2024-01-15T10:32:00Z"
}
```

## WebSocket Support

The gateway supports WebSocket connections for real-time communication.

### Connect

```javascript
const ws = new WebSocket('ws://localhost:8081/ws');

ws.onopen = function() {
    console.log('Connected to gateway');
};

ws.onmessage = function(event) {
    const message = JSON.parse(event.data);
    console.log('Received:', message);
};
```

### Send Message

```javascript
const message = {
    type: 'task',
    data: {
        role: 'user',
        parts: [
            {
                type: 'text',
                content: 'Hello from WebSocket'
            }
        ]
    }
};

ws.send(JSON.stringify(message));
```

### Message Types

#### Task Message

```json
{
  "type": "task",
  "data": {
    "role": "user",
    "parts": [
      {
        "type": "text",
        "content": "Process this request"
      }
    ]
  }
}
```

#### Status Update

```json
{
  "type": "status",
  "taskId": "task-123",
  "status": "processing",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

#### Result Message

```json
{
  "type": "result",
  "taskId": "task-123",
  "data": {
    "type": "text",
    "content": "Task completed successfully"
  }
}
```

## Error Handling

### Error Response Format

```json
{
  "error": {
    "code": "SERVICE_UNAVAILABLE",
    "message": "No healthy services available",
    "details": {
      "requested_skill": "process_reimbursement",
      "available_services": 0
    },
    "timestamp": "2024-01-15T10:30:00Z"
  }
}
```

### HTTP Status Codes

- `200 OK` - Request successful
- `400 Bad Request` - Invalid request format
- `401 Unauthorized` - Authentication required
- `403 Forbidden` - Access denied
- `404 Not Found` - Resource not found
- `429 Too Many Requests` - Rate limit exceeded
- `500 Internal Server Error` - Gateway error
- `502 Bad Gateway` - Backend service error
- `503 Service Unavailable` - No services available
- `504 Gateway Timeout` - Request timeout

### Error Codes

- `INVALID_REQUEST` - Malformed request
- `AUTHENTICATION_FAILED` - Invalid credentials
- `AUTHORIZATION_FAILED` - Insufficient permissions
- `SERVICE_NOT_FOUND` - Requested service not available
- `SERVICE_UNAVAILABLE` - All services unhealthy
- `TIMEOUT` - Request timeout
- `RATE_LIMIT_EXCEEDED` - Too many requests
- `INTERNAL_ERROR` - Gateway internal error

## Rate Limiting

The gateway implements rate limiting to protect backend services.

### Headers

Rate limit information is included in response headers:

```http
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1642248600
```

### Rate Limit Response

When rate limit is exceeded:

```http
HTTP/1.1 429 Too Many Requests
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1642248600

{
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "Rate limit exceeded. Try again later.",
    "details": {
      "limit": 1000,
      "window": "1h",
      "reset_at": "2024-01-15T11:00:00Z"
    }
  }
}
```

## Request/Response Examples

### Routing Based on Skills

```http
POST /tasks/send
Content-Type: application/json
X-Skill-Required: process_reimbursement

{
  "role": "user",
  "parts": [
    {
      "type": "text",
      "content": "I need to submit an expense report"
    }
  ]
}
```

### Load Balancing

The gateway automatically distributes requests across healthy services:

```http
# Request 1 -> Agent A
# Request 2 -> Agent B  
# Request 3 -> Agent A
# etc.
```

### Health Check Integration

Services are automatically removed from load balancing when unhealthy:

```json
{
  "services": [
    {
      "name": "agent-1",
      "status": "healthy",
      "last_health_check": "2024-01-15T10:29:00Z"
    },
    {
      "name": "agent-2", 
      "status": "unhealthy",
      "last_health_check": "2024-01-15T10:28:00Z"
    }
  ]
}
```
