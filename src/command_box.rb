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
    @num_filtered = 0
    @response_body = nil
    @request = request
    @body = request["body"]
    @script = request["script"]
    @global = request["global"]
    @selected = request["selected"]
  end

  def preview
    items = []
    case @body["type"]
    when "select_file"
      @body["files"].each do |file|
        items << { type: "print", body: file["display_name"] }
      end
    when "select_match"
      @body["locations"].each do |loc|
        y = loc["range"]["start"]["y"]
        line = File.read(loc["file"]["path"]).lines[y] || ""
        body = line
        items << {
          type: "print_with_file",
          file: loc["file"],
          lineno: y + 1,
          body: body,
        }
      end
    else
      @message = "ruby: unknown request type '#{@body['type']}'"
    end

    @response_body = {
      type: "preview",
      items: items,
    }
  end

  def commit
    case @body["type"]
    when "select_file"
      type = "goto"
      file = @body["files"][@selected]
      @response_body = {
        type: "goto",
        file: file,
      }
    when "select_match"
      type = "goto"
      loc = @body["locations"][@selected]
      @response_body = {
        type: "goto",
        file: loc["file"],
        position: loc["range"]["start"]
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
      num_filtered: @num_filtered,
      body: @response_body,
    }
  end
end

request = JSON.parse(STDIN.read)
response = Executor.new(request).run
puts response.to_json
