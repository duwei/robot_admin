# Robot Admin 系统序列图

## 客户端注册和状态更新流程

```mermaid
sequenceDiagram
    participant Client as 游戏客户端
    participant Server as Robot Admin 服务器
    participant Web as Web 前端

    %% 客户端注册
    Client->>Server: RegisterClient(client_id, name, type, version)
    Server-->>Client: RegisterResponse(success)
    
    %% 状态更新循环
    loop 每5秒
        Client->>Server: UpdateStatus(client_id, metrics)
        Server-->>Client: StatusUpdateResponse
        
        alt 有新命令
            Server-->>Client: 返回新命令
            Client->>Client: 执行命令
            Client->>Server: 更新命令执行状态
        end
    end

    %% Web前端交互
    Web->>Server: GET /api/clients
    Server-->>Web: 返回客户端列表和状态
    
    Web->>Server: POST /api/commands
    Server->>Server: 保存命令到客户端状态
    Server-->>Web: 返回命令ID
```

## 命令执行流程

```mermaid
sequenceDiagram
    participant Web as Web 前端
    participant Server as Robot Admin 服务器
    participant Client as 游戏客户端

    %% 发送命令
    Web->>Server: POST /api/commands {client_id, command, parameters}
    Server->>Server: 生成命令ID
    Server->>Server: 保存命令到客户端状态
    Server-->>Web: 返回命令ID

    %% 客户端获取并执行命令
    Client->>Server: UpdateStatus(client_id, metrics)
    Server-->>Client: 返回待执行命令
    Client->>Client: 执行命令
    
    %% 更新命令状态
    Client->>Server: UpdateStatus(command_id, status)
    Server->>Server: 更新命令状态
    Server-->>Client: StatusUpdateResponse

    %% Web前端获取更新
    Web->>Server: GET /api/clients
    Server-->>Web: 返回更新后的客户端状态
```

## 组件交互说明

1. **客户端注册流程**
   - 客户端启动时向服务器注册
   - 提供客户端ID、名称、类型和版本信息
   - 服务器确认注册并保存客户端信息

2. **状态更新机制**
   - 客户端每5秒向服务器发送一次状态更新
   - 更新包含：
     - 客户端指标（CPU、内存等）
     - 当前命令执行状态（如果有）
   - 服务器通过状态更新响应下发新命令

3. **命令执行流程**
   - Web前端发送命令到服务器
   - 服务器保存命令到对应客户端的状态
   - 客户端在下次状态更新时接收命令
   - 客户端执行命令并报告执行状态
   - Web前端通过轮询获取最新状态

4. **错误处理**
   - 客户端连接断开时自动重连
   - 命令执行失败时报告错误状态
   - 服务器保持命令状态直到确认完成或失败
