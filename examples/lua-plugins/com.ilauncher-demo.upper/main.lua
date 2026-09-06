-- Upper 命令：参数转大写并复制到剪贴板
-- 演示：前缀命令触发、preview/run 协议、clipboard:write 权限声明

function preview(args, selection)
  if #args == 0 then
    return "upper <文本>", "参数转大写并复制到剪贴板"
  end
  return "转大写并复制", table.concat(args, " ")
end

function run(args, selection)
  if #args == 0 then
    return "用法: upper <文本>"
  end
  ilauncher.copy(string.upper(table.concat(args, " ")))
  return "已复制"
end
