require "json"
require "pp"

def pp(obj)
  PP.pp(obj, STDERR)
end

def str_execute(_, code)
  puts _.instance_eval(&code)
rescue Errno::EPIPE
  exit
end

class Executor
  def initialize(request)
    @message = nil
    @response_body = nil
    @request = request
    @body = request["body"]
    @script = request["script"]
    @global = request["global"]
    @selected = request["selected"]
  end

  def preview
    items = []
    selectable = false

    case @body["type"]
    when "select_file"
      selectable = true
      @body["files"].each do |file|
        items << { type: "print", body: file["display_name"] }
      end
    when "select_match"
      selectable = true
      @body["locations"].each do |loc|
        y = loc["range"]["start"]["y"]
        line = File.read(loc["file"]["path"]).lines[y] || ""
        items << {
          type: "print_with_file",
          file: loc["file"],
          lineno: y + 1,
          body: line,
        }
      end
    when "goto"
      line = File.read(@body["path"]).lines[@body["point"]["y"]] || ""
      items << { type: "print", body: line }
    when "replace_with"
      @body["locations"].each do |loc|
        y = loc["range"]["start"]["y"]
        line = File.read(loc["file"]["path"]).lines[y] || ""
        x_range = loc["range"]["start"]["x"] ... loc["range"]["end"]["x"]
        pp x_range
        line[x_range] = @body["new_str"]
        items << {
          type: "print_with_file",
          file: loc["file"],
          lineno: y + 1,
          body: line,
        }
      end
    else
      @message = "ruby: unknown request type '#{@body['type']}'"
    end

    @response_body = {
      type: "preview",
      items: items,
      selectable: selectable,
    }
  end

  def commit
    case @body["type"]
    when "select_file"
      @response_body = {
        type: "goto",
        file: @body["files"][@selected],
      }
    when "select_match"
      loc = @body["locations"][@selected]
      @response_body = {
        type: "goto",
        file: loc["file"],
        position: loc["range"]["start"]
      }
    when "goto"
      @response_body = {
        type: "goto",
        file: @body["file"],
        position: @body["point"]
      }
    when "replace_with"
      changes = []
      @body["locations"].each do |loc|
        changes << {
          location: loc,
          new_str: @body["new_str"],
        }
      end
      @response_body = {
        type: "replace_with",
        changes: changes,
      }
    end
  end

  def run
    if @request["preview"]
      preview
    else
      commit
    end

    {
      message: @message,
      body: @response_body,
    }
  end
end

request = JSON.parse(STDIN.read)
response = Executor.new(request).run
puts response.to_json
