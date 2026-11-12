*关于本项目：HGhubX 只是我体验 Tauri2 移动端开发时做的小玩具，开源几天后不知为何收到了大量 star。*

*意识到可能存在的潜在危险后，我立刻更换了开源协议为 **PolyForm Noncommercial 1.0.0**，并在 Readme 中写入了大量保护自己的文段。目前 HGhubX 收到的 star 实际上德不配位。*

*在之后的更新中，我不再会维护任何获取上游数据的工作，维护方向将偏重于 UI/UX 与使用体验，同时仍会低频率的响应 PR 与 Issues。如果版权方或其授权代表认为本项目涉及其合法权益，欢迎通过 GitHub 仓库留言联系。收到明确的权利方通知后，我会第一时间积极响应，并根据对方诉求立即归档项目或将仓库转为私有，停止公开访问。*

<div align="center">

# HGhubX

**HGhubX 第三方短剧客户端 · Rust + Tauri 2**

Windows / macOS / Android 三端 · Fluent 设计 · 沉浸式竖屏播放

</div>

> ⚠️ 本项目仅供 **学习与技术交流**，不提供、不存储、不分发任何音视频内容。
> 所有内容均来自第三方站点，版权归原作者与上游站点所有。

**使用须知**

- **以访客身份正常使用**：像普通访客一样浏览就好，请勿滥用本项目。
- **不要给上游增加压力**：本项目本身不提供下载功能，请勿改动代码去做批量下载 / 批量抓取，也不要高频轮询、并发轰炸接口或挂机刷播放量；内置的封面与播放列表缓存请不要改小或关掉。
- **尊重上游版权**：不要去除水印与站点标识，不要把剧集搬运、转码、修改后二次分发到别处（网盘、资源站、其他 App 等），更不能改完拿出去出售。
- **不要公开传播接口与直链**：接口地址、播放直链、解密方式请勿公开张贴或打包成「资源站」——上游一旦封禁，所有人都没法再用。
- **不要商业化**：禁止打包出售、接入广告 / 会员 / 付费点播，或作为收费服务的一部分分发（详见 [LICENSE](LICENSE)）。
- **合理使用**：仅供个人学习、研究与自用；请在 24 小时内删除相关资源，并支持正版。
- **遵守当地法律**：使用者应自行判断并承担合规责任。

> ⚖️ 许可证自 v1.1.0 版本起改为 **PolyForm Noncommercial 1.0.0**：**禁止任何商业用途**——
> 包括打包出售、接入付费/会员/广告变现、作为收费产品或服务的一部分分发。
> 个人学习、研究、自用与非营利使用不受限制，详见 [LICENSE](LICENSE)。

## Docker Web 版

在部署主机上克隆仓库，然后在仓库根目录创建 `.env`。把示例值替换成独立的随机密码（至少 16 个可打印 ASCII 字符，不含空格）。可选的 `HGHUBX_WEB_PORT` 用于更改宿主机端口。

```dotenv
HGHUBX_WEB_PASSWORD='replace-with-a-random-password'
HGHUBX_WEB_PORT=8080
```

```sh
docker compose up -d --build
docker compose ps
```

在部署主机打开 [http://127.0.0.1:8080](http://127.0.0.1:8080)，在浏览器登录提示里输入用户名 `admin` 和上面设置的密码。首次进入后，在页面里填写自己的上游接入地址。

**远程访问。** Compose 默认只在部署主机的回环地址监听。如果主机已启用 SSH，可以在自己的电脑运行下面的命令，然后用本机浏览器打开同一个地址。将 `user@server` 替换为你的 SSH 用户和主机地址。

```sh
ssh -N -L 127.0.0.1:8080:127.0.0.1:8080 user@server
```

如果主机和手机已加入同一个 Tailscale 网络，在部署主机运行 `tailscale serve --bg 8080`，再从手机打开命令输出的 HTTPS 地址。[Tailscale 的 Docker 部署示例](https://tailscale.com/docs/solutions/host-private-server-docker)

已有 HTTPS 反向代理也可以转发到部署主机的 `127.0.0.1:8080`。代理须保留请求的 `Authorization` 头，并将整个站点挂在域名根路径。请使用私有入口，并通过 HTTPS 或 SSH 隧道传输浏览器登录凭据。

这是单用户部署。所有通过入口认证的设备共享接入地址和上游登录状态，这些数据保存在 `hghubx-data` Docker 卷里。收藏、观看历史和界面偏好保存在各自的浏览器。更新代码后再次运行 `docker compose up -d --build` 即可。

### 从 Unraid 拉取镜像

`main` 分支推送后，GitHub Actions 会构建和验证 Linux x86-64 镜像，再发布到 GHCR。

```sh
docker pull ghcr.io/nradar/hghubx-docker:latest
```

在 Unraid 终端创建持久化目录，让容器用户可以写入：

```sh
install -d -o 10001 -g 10001 -m 0700 /mnt/user/appdata/hghubx/state
```

在 **Docker → Add Container** 中填写以下配置。

| 项目 | 值 |
| --- | --- |
| Name | `hghubx` |
| Repository | `ghcr.io/nradar/hghubx-docker:latest` |
| Network Type | `Bridge` |
| TCP Port | 宿主机 `18080` → 容器 `8080` |
| Path | `/mnt/user/appdata/hghubx/state` → `/data/state`，读写 |
| Variable | `HGHUBX_WEB_PASSWORD`，填入至少 16 位的独立随机密码 |

在局域网打开 `http://<Unraid-IP>:18080`，以 `admin` 登录。跨网络访问时使用私有 HTTPS 入口。

## 仿冒提醒

> 🚫 **已发现的恶意修改版**：[`hichuv/HGhub`](https://github.com/hichuv/HGhub)
>
> 该仓库与本项目无关，是在旧版本源码基础上擅自加入**卡密验证、VIP 付费解锁**等变现功能的修改版，
> 代码改动不透明、安全性无法验证。**请勿下载、勿注册、勿付费**；如已安装，建议立即卸载，改用官方 Release 包安装。
>
> 本项目**从未、也不会**加入任何卡密 / VIP / 付费解锁 / 广告功能。如果你遇到的「HGhubX」要求输入卡密或付费开通，那一定不是官方版本。

## 免责声明

- 本项目与内容提供方无任何关联，作者不参与内容的制作、上传与分发。
- 本项目仅在上述官方渠道发布；任何第三方修改版（尤其是加卡密、VIP 变现的版本）均与作者无关，请自行甄别。
- 使用者应自行遵守所在地法律法规；因使用本项目产生的任何后果由使用者自行承担。
- 若相关权利方认为本项目存在侵权，请通过 Issue 联系，我们会及时处理。

## License

[PolyForm Noncommercial License 1.0.0](LICENSE) —— **仅限非商业用途**。

- **允许**：个人学习、研究、自用、爱好项目，以及在非营利组织内部使用；可以自由修改、二次开发。
- **禁止**：任何商业用途。包括但不限于打包出售、接入付费/会员/广告变现、作为收费服务或商业产品的一部分分发。
- **保留声明**：分发本项目或其修改版时，必须附带本许可证全文（或上面的链接）以及 `Required Notice` 声明行。
- 商业授权请通过 Issue 联系作者单独洽谈。
