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
    if @script.empty?
      case @body["type"]
      when "files"
        items = []
        @body["files"].each do |file|
          items << { type: "print", body: file["display_name"] }
        end
        @response_body = {
          type: "preview",
          items: items,
        }
      end
    else
    end
  end

  def commit
    if @script.empty?
      case @body["type"]
      when "files"
        type = "goto"
        file = @body["files"][@selected]
        @response_body = {
          type: "goto",
          file: file,
        }
      end
    else
    end
  end

  def run
    pp @request
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
pp response
puts response.to_json
